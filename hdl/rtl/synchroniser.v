`default_nettype none
// Synchroniser
//
// Brings signals that come from outside the FPGA (or from another clock
// domain) safely into the `clk` domain. Each bit passes through a chain of
// STAGES flip-flops. If the first flip-flop samples an input just as it
// changes and goes metastable (stuck between 0 and 1), it has a full clock
// cycle to settle before the next stage samples it, so a clean value comes out.
//
// WARNING: only for independent single-bit signals (e.g. an ADC busy pin, a
// fault input). Never pass a multi-bit value such as a number or counter
// through this: each bit can settle a cycle apart from the others, giving a
// value that is half old and half new. Multi-bit data needs a handshake.
//
// Parameters:
//   WIDTH   number of independent 1-bit signals (default 1)
//   STAGES  flip-flops per signal, must be at least 2 (default 2)
//
// Ports:
//   clk       system clock
//   rst       synchronous, active-high reset; clears every stage to 0
//   async_in  [WIDTH-1:0] asynchronous inputs, one per signal
//   sync_out  [WIDTH-1:0] synchronised outputs, one per signal
//
// Latency: STAGES clock cycles from async_in to sync_out.
// Must not be followed by any logic between the stages.

module synchroniser #(parameter WIDTH = 1, parameter STAGES = 2)(
    input   wire                    clk,
    input   wire                    rst,
    input   wire [WIDTH - 1 : 0]    async_in,
    output  wire [WIDTH - 1 : 0]    sync_out
);

    reg [WIDTH - 1 : 0] ff [0 : STAGES - 1];

    integer stage_count;

    always @(posedge clk) begin
        if (rst) begin
            for (stage_count = 0; stage_count < STAGES; stage_count = stage_count + 1) begin
                ff[stage_count] <= {WIDTH{1'b0}};
            end
        end 
        else begin
            ff[0] <= async_in;
            for (stage_count = 1; stage_count < STAGES; stage_count = stage_count + 1) begin
                ff[stage_count] <= ff[stage_count - 1];
            end
        end
    end 
    assign sync_out = ff[STAGES - 1];
endmodule
`default_nettype wire
