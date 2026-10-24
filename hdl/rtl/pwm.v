// PWM generator (centred pulses)
//
// Produces a pulse-width modulated output at a fixed frequency, with each pulse
// centred in its period. Because the pulse is always centred, the middle of the
// on-time falls at the same point in every period whatever the duty, which is
// where the coil current should be sampled (it equals the average current there).
//
// Parameters:
//   CLK_FREQ_KHZ  clock frequency in kHz (default 100_000 = 100 MHz)
//   PWM_FREQ_KHZ  PWM frequency in kHz (default 20)
//   WIDTH         width of `duty`; derived so it can hold COUNT, leave as default
//
//   COUNT = CLK_FREQ_KHZ / PWM_FREQ_KHZ   clock cycles per period (5000 by default)
//
// Ports:
//   clk      clock at CLK_FREQ_KHZ
//   rst      synchronous, active-high reset; output low, duty 0 until set
//   duty     [WIDTH-1:0] number of high cycles per period, 0 to COUNT.
//            0 = always off; COUNT or more = always on (clamped, never wraps)
//   enable   low: output forced low on the next clock edge. The counter keeps
//            running, so the PWM stays in step with the rest of the system.
//            High again: output resumes at the start of the next period, so
//            the first pulse after a fault is always a complete one.
//   pwm_out  the PWM output
//   centre_pulse  one-cycle pulse at the middle of each period (counter = (COUNT-1)/2),
//                 i.e. the middle of the on-time whatever the duty. Pulses every period,
//                 even when disabled or at 0%: it is the timing reference for sampling
//                 the coil current.
//
// Timing:
//   - A new `duty` is only picked up at the start of each period (shadow
//     register), so changing it mid-period can never produce a malformed pulse.
//   - The pulse occupies counter values start .. start + duty - 1, where
//     start = (COUNT - duty) / 2. When COUNT - duty is odd, the off-time after
//     the pulse is one cycle longer than the off-time before it.
//   - pwm_out is registered: it changes one clock cycle after the counter value
//     that causes the change.
`default_nettype none
module pwm #(
    parameter PWM_FREQ_KHZ  = 20,          // KHz
    parameter CLK_FREQ_KHZ  = 100_000,
    parameter WIDTH         = $clog2(CLK_FREQ_KHZ / PWM_FREQ_KHZ + 1)
) (
    input  wire                 clk,      
    input  wire                 rst,      // synchronous, active high
    input  wire [WIDTH - 1 : 0] duty,
    input  wire                 enable,
    output reg                  pwm_out,
    output reg                  centre_pulse
);

    localparam integer COUNT_INT = CLK_FREQ_KHZ / PWM_FREQ_KHZ;
    localparam [WIDTH - 1 : 0] COUNT = COUNT_INT[WIDTH - 1 : 0];
    localparam [WIDTH - 1 : 0] CENTRE = (COUNT - 1) / 2;


    reg [WIDTH - 1 : 0] counter;
    reg [WIDTH - 1 : 0] duty_latched;
    reg [WIDTH - 1 : 0] CA_BUFFER;
    reg                 enable_period;


    always @(posedge clk) begin
        if (rst) begin
            counter         <= {WIDTH{1'b0}};
            duty_latched    <= {WIDTH{1'b0}};
            CA_BUFFER       <= {WIDTH{1'b0}};
            enable_period   <= 1'b0;
            centre_pulse    <= 1'b0;
            pwm_out         <= 1'b0;
        end else begin
            if (counter == COUNT - 1) begin
                duty_latched    <= duty;
                CA_BUFFER       <= (duty < COUNT) ? (COUNT - duty) / 2 : 0;
                counter         <= {WIDTH{1'b0}};
                enable_period   <= enable;
            end else begin
                counter <= counter + 1'b1; 
            end
            centre_pulse <= (counter == CENTRE);
            pwm_out <= enable & enable_period
                     & (counter >= CA_BUFFER)
                     & (counter < duty_latched + CA_BUFFER);
            end
        end
endmodule
`default_nettype wire
