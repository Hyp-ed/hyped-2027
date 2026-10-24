`default_nettype none
package fixed_point_pkg;
    localparam int WIDE_W = 64;
    typedef logic signed [WIDE_W - 1 : 0] wide_t;

    /* verilator lint_off UNUSEDPARAM */

    // PROVISIONAL (2026-10-22): assumes coil current stays within ±32 A.
    // Confirm max current with Emag (Linear D2), then revisit.
    localparam int CURRENT_W    = 16;   // Q6.10: ±32 A, ~1 mA resolution
    localparam int CURRENT_FRAC = 10;
    typedef logic signed [CURRENT_W-1:0] current_t;

    // PROVISIONAL (2026-10-22): PI output is PWM duty in clock cycles.
    localparam int OUTPUT_W    = 16;   // Q16.0: ±32767 cycles
    localparam int OUTPUT_FRAC = 0;
    typedef logic signed [OUTPUT_W-1:0] output_t;

    // PROVISIONAL (2026-10-22): gain range not yet known; revisit with Jason's gain values.
    localparam int GAIN_W    = 32;   // Q16.16: ±32768, ~0.000015 resolution
    localparam int GAIN_FRAC = 16;
    typedef logic signed [GAIN_W-1:0] gain_t;

    /* verilator lint_on UNUSEDPARAM */

    function automatic wide_t round_shift(input wide_t x, input int shift);
        wide_t result;
        logic guard_bit;

        if(shift <= 0)
            return x;

        guard_bit = x[shift - 1];
        result = x >>> shift;

        if (guard_bit)
            result = result + 1;
        
        return result;

    endfunction

    function automatic wide_t saturate(input wide_t x, input int width);
        wide_t max_val;  
        wide_t min_val;
        wide_t result;

        max_val = (wide_t'(1) <<< (width - 1)) - 1;  
        min_val = ~max_val;

        if (x < min_val) 
            result = min_val;
        else if (x > max_val)
            result = max_val;
        else
            result = x;

        return result;

    endfunction

    function automatic wide_t s_round_shift(input wide_t x, input int shift, input int width);
        wide_t shift_result;
        wide_t result;
        shift_result = round_shift(x, shift);
        result = saturate(shift_result, width);

        return result;

    endfunction

    function automatic wide_t clamp(input wide_t x, input wide_t min_val, input wide_t max_val);        
        if(x < min_val)
            return min_val;
        else if (x > max_val)
            return max_val;
        else 
            return x;

    endfunction

endpackage
`default_nettype wire
