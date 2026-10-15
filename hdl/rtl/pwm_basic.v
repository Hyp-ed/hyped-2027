// Basic PWM generator (workshop version)
//
// Produces a pulse-width modulated output: a square wave whose "high" fraction
// (the duty cycle) is set by the `duty` input.
//
//   period      = 2^WIDTH clock cycles
//   high cycles = duty   (so duty = 64 with WIDTH = 8 gives 64/256 = 25% high)
//
// A new `duty` value is only picked up at the start of each period. Without
// this, changing `duty` mid-period could produce a glitchy, shortened pulse.

module pwm_basic #(
    parameter WIDTH = 8
) (
    input  wire             clk,
    input  wire             rst,      // synchronous, active high
    input  wire [WIDTH-1:0] duty,
    output reg              pwm_out
);

    reg [WIDTH-1:0] counter;
    reg [WIDTH-1:0] duty_latched;

    always @(posedge clk) begin
        if (rst) begin
            counter      <= {WIDTH{1'b0}};
            duty_latched <= {WIDTH{1'b0}};
            pwm_out      <= 1'b0;
        end else begin
            counter <= counter + 1'b1;  // wraps back to 0 after 2^WIDTH - 1

            // Last cycle of the period: take the new duty for the next period
            if (counter == {WIDTH{1'b1}})
                duty_latched <= duty;

            pwm_out <= (counter < duty_latched);
        end
    end

endmodule
