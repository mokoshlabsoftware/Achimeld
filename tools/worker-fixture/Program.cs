using System.Buffers.Binary;
using System.Diagnostics;
using System.IO.Pipes;
using System.Text.Json;
using Achimeld.ReadWorker;

// Test peer only. Production never accepts this executable or any arbitrary worker path.
if (args.SequenceEqual(new[] { "--read-steam", "480" })) return;
if(args.Length==2 && args[1]=="run-driver") {
    if(args[0]=="hang.js"){await Task.Delay(Timeout.Infinite);return;}
    using var stdin=Console.OpenStandardInput();using var output=Console.OpenStandardOutput();using var error=Console.OpenStandardError();
    byte[] size=new byte[4];await stdin.ReadExactlyAsync(size);int count=BinaryPrimitives.ReadInt32LittleEndian(size);
    if(count<1 || count>32*1024*1024)throw new InvalidDataException();
    byte[] bytes=new byte[count];await stdin.ReadExactlyAsync(bytes);await output.WriteAsync(size);await output.WriteAsync(bytes);
    await error.WriteAsync("fixture stderr"u8.ToArray());return;
}
if(args.Length==1 && args[0]=="--child") { await Task.Delay(Timeout.Infinite);return; }
if(args.Length!=1 || !args[0].StartsWith("Achimeld.Steam.",StringComparison.Ordinal)) throw new ArgumentException();
using var pipe=new NamedPipeClientStream(".",args[0],PipeDirection.InOut,PipeOptions.Asynchronous);
await pipe.ConnectAsync(10000);
byte[] header=new byte[4];await pipe.ReadExactlyAsync(header);
int length=BinaryPrimitives.ReadInt32LittleEndian(header);
if(length<1 || length>32*1024*1024) throw new InvalidDataException();
byte[] input=new byte[length];await pipe.ReadExactlyAsync(input);
using var request=JsonDocument.Parse(input);
string operation=request.RootElement.GetProperty("operation").GetString()!;
if(operation=="diagnostic-exit") { Environment.Exit(209); return; }
if (operation is not ("large" or "child" or "hang" or "oversize")) {
    // Test the exact production receiving policy over a real pipe, without Steam.
    var policy = new ReadOnlyPolicy(); policy.Validate("start", JsonSerializer.SerializeToElement<object?>(null), true);
    bool allowed = false;
    try { policy.Validate(operation, request.RootElement.GetProperty("arguments"), true); allowed = true; }
    catch (InvalidDataException) { }
    byte[] guarded = JsonSerializer.SerializeToUtf8Bytes(new {requestId=request.RootElement.GetProperty("requestId").GetString(),
        success=allowed, data=allowed, errorCode=allowed ? null : "steam.read.failed"});
    BinaryPrimitives.WriteInt32LittleEndian(header,guarded.Length); await pipe.WriteAsync(header); await pipe.WriteAsync(guarded);
    await Task.Delay(Timeout.Infinite); return;
}
if(operation=="hang") { await Task.Delay(Timeout.Infinite);return; }
if(operation=="oversize") { BinaryPrimitives.WriteInt32LittleEndian(header,32*1024*1024+1);await pipe.WriteAsync(header);await Task.Delay(Timeout.Infinite);return; }
int childPid=0;
if(operation=="child") {
    var start=new ProcessStartInfo(Environment.ProcessPath!) {UseShellExecute=false,CreateNoWindow=true};start.ArgumentList.Add("--child");
    using var child=Process.Start(start)!;childPid=child.Id;
}
byte[] response=JsonSerializer.SerializeToUtf8Bytes(new {requestId=request.RootElement.GetProperty("requestId").GetString(),success=true,
    data=new {text=operation=="large" ? string.Concat(Enumerable.Repeat("Żółw🦀",40000)) : "ok",childPid},errorCode=(string?)null});
BinaryPrimitives.WriteInt32LittleEndian(header,response.Length);
// Fragment the header and the payload across UTF-8 character boundaries.
foreach(byte value in header) await pipe.WriteAsync(new byte[] {value});
for(int offset=0;offset<response.Length;offset+=997) await pipe.WriteAsync(response.AsMemory(offset,Math.Min(997,response.Length-offset)));
await Task.Delay(Timeout.Infinite);
