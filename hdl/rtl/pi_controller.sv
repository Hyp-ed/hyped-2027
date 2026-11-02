// PI controller (fixed point)
//
// Generic proportional-integral controller, run once per new measurement. Used
// for the coil current loop now, and reusable for other loops later because the
// gains and output limits are runtime inputs rather than built-in constants.
//
// Each update (started by a one-cycle `sample_valid` pulse):
//   error      = setpoint - measured
//   p          = kp * error
//   increment  = ki * error            (ki is the per-sample gain, Ki x T,
//                                       computed offline: no time units here)
//   integrator = integrator + increment, with anti-windup (below)
//   out        = clamp(p + integrator, out_min, out_max)
// The output uses the integrator after this update, so the current sample's
// error is included.
//
// Anti-windup (conditional integration plus clamping):
//   - The increment is skipped if it would push the output further into
//     saturation: output above out_max with a positive increment, or below
//     out_min with a negative increment.
//   - Otherwise the integrator is updated and clamped to [out_min, out_max].
//   - While `clear` is high the integrator is held at 0, so it cannot wind up
//     while the power stage is disabled.
// The integrator accumulates ki * error rather than raw error, so changing ki
// mid-run does not make the output jump.
//
// Number formats (from fixed_point_pkg, provisional until confirmed):
//   setpoint, measured   current_t   Q6.10, amps
//   kp, ki               gain_t      Q16.16, output units per amp
//   out_min, out_max,    output_t    Q16.0, PWM duty in clock cycles
//   out
// Internally: error has one extra bit (Q7.10) because subtracting two currents
// can exceed the current range; products are Q.26 (10 + 16 fraction bits); the
// integrator keeps 16 fraction bits (INT_FRAC) so small errors still accumulate.
// Narrowing rounds to nearest (halves round up) and saturates; nothing wraps.
//
// Ports:
//   clk           clock
//   rst           synchronous, active-high reset: integrator, outputs and
//                 pipeline cleared
//   clear         while high, the integrator is held at 0
//   sample_valid  one-cycle pulse: setpoint and measured are valid, run one update
//   setpoint      target current
//   measured      measured current
//   kp, ki        gains
//   out_min,      output limits; out_min must not exceed out_max
//   out_max
//   out           controller output
//   out_valid     one-cycle pulse when `out` has been updated
//   saturated     high if the last update's output hit out_min or out_max
//
// Timing:
//   - kp, ki, out_min and out_max are captured when sample_valid arrives, so
//     changing them mid-calculation cannot mix old and new values.
//   - Latency: 4 clock cycles from sample_valid to out_valid. A new
//     sample_valid must not arrive before the previous update has finished.

module pi_controller 
    import fixed_point_pkg::*;
(
    input  logic     clk,
    input  logic     rst,           // synchronous, active high
    input  logic     clear,         // while high: integrator held at 0 (stage disabled)
    input  logic     sample_valid,  // one-cycle pulse: new measurement, run one update
    input  current_t setpoint,
    input  current_t measured,
    input  gain_t    kp,
    input  gain_t    ki,            // per-sample gain: Ki × T, computed offline
    input  output_t  out_min,
    input  output_t  out_max,
    output output_t  out,
    output logic     out_valid,     // one-cycle pulse when `out` is updated
    output logic     saturated      // the last output hit a limit
);

    localparam int ERR_W     = CURRENT_W + 1;            // one spare bit for the subtraction
    localparam int PROD_W    = ERR_W + GAIN_W;
    localparam int PROD_FRAC = CURRENT_FRAC + GAIN_FRAC; 
    localparam int INT_FRAC  = 16;                       
    localparam int INT_W     = OUTPUT_W + INT_FRAC + 2;  
    typedef logic signed [PROD_W - 1 : 0] prod_t;
    typedef logic signed [INT_W - 1 : 0] int_t;
    typedef logic signed [ERR_W - 1 : 0] err_t;

    err_t           error;
    gain_t          kp_q, ki_q;
    output_t        out_min_q, out_max_q;
    prod_t          p_prod, i_prod;
    int_t           integrator;
    logic           valid_0, valid_1, valid_2;
    wide_t          p_scaled;

    always_ff @(posedge clk) begin
        if (rst) begin
            valid_0     <= 1'b0;
            valid_1     <= 1'b0;
            valid_2     <= 1'b0;
            out_valid   <= 1'b0;
            saturated   <= 1'b0;
            error       <= '0;
            kp_q        <= '0;
            ki_q        <= '0;
            out_min_q   <= '0;
            out_max_q   <= '0;
            integrator  <= '0;
            p_prod      <= '0;
            i_prod      <= '0;
            p_scaled    <= '0;
            out         <= '0;
        end else begin
            valid_0 <= sample_valid;
            if (sample_valid) begin
                error     <= ERR_W'(setpoint) - ERR_W'(measured);
                kp_q      <= kp;
                ki_q      <= ki;
                out_min_q <= out_min;
                out_max_q <= out_max;
            end
            valid_1 <= valid_0;
            if (valid_0) begin
                p_prod    <= error * kp_q;
                i_prod    <= error * ki_q;
            end
            valid_2 <= valid_1;
            
            if (valid_1) begin 
                wide_t  p_term, increment, candidate, test_out, hi, lo;
                p_term      = round_shift(wide_t'(p_prod), PROD_FRAC - INT_FRAC); // Take Q23.26 -> Q23.16
                increment   = round_shift(wide_t'(i_prod), PROD_FRAC - INT_FRAC);
                hi          = wide_t'(out_max_q) <<< INT_FRAC;
                lo          = wide_t'(out_min_q) <<< INT_FRAC;
                candidate   = wide_t'(integrator) + increment;
                test_out    = p_term + candidate;

                if (!(test_out > hi && increment > 0) && !(test_out < lo && increment < 0))
                    integrator  <= INT_W'(clamp(candidate, lo, hi));

                
                p_scaled    <= p_term;
            end
            if (clear)
                integrator  <= 0;
            
            if (valid_2) begin
                wide_t scaled_sum, rounded, clamped;
                scaled_sum   = p_scaled + wide_t'(integrator);
                rounded      = round_shift(wide_t'(scaled_sum), INT_FRAC - OUTPUT_FRAC);   // Q.16 -> Q.0
                clamped      = clamp(rounded, wide_t'(out_min_q), wide_t'(out_max_q));
                out          <= output_t'(clamped);
                saturated    <= (rounded != clamped);
            end
            out_valid <= valid_2;


        end
    end
endmodule
