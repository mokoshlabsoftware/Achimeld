using System.Diagnostics;
using System.Security.Cryptography;
using System.Text.Json;

if (args.Length != 3 || args[0] != "--development") throw new ArgumentException("Only --development REPOSITORY PACKAGE is supported.");
string repository = Path.GetFullPath(args[1]), package = Path.GetFullPath(args[2]);
string artifacts = Path.GetFullPath(Environment.GetEnvironmentVariable("ACHIMELD_ARTIFACT_ROOT") ?? throw new InvalidOperationException("Use build/BridgePackage.ps1."));
string target = Path.GetFullPath(Environment.GetEnvironmentVariable("CARGO_TARGET_DIR") ?? throw new InvalidOperationException("Rust output directory unavailable."));
string boundary = artifacts + Path.DirectorySeparatorChar;
if (!package.StartsWith(boundary,StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("Package boundary rejected.");
using RSA key = RSA.Create(4096);
byte[] publicKey = key.ExportSubjectPublicKeyInfo();
string keyId = Convert.ToHexStringLower(SHA256.HashData(publicKey));
string trust = Path.Combine(artifacts,"bridge-trust-" + keyId[..16] + ".json");
File.WriteAllText(trust,JsonSerializer.Serialize(new { mode="development",minimumVersion="0.1.0",publisher=(string?)null,
    keys=new[] {new {keyId,publicKey=Convert.ToBase64String(publicKey),revoked=false}}}));
// The ephemeral private signing key is never written to disk or given to build children.
var build = new ProcessStartInfo("pwsh.exe") { WorkingDirectory=repository,UseShellExecute=false,CreateNoWindow=true };
foreach (string argument in new[] {"-NoProfile","-File",Path.Combine(repository,"build","Rust.ps1"),"-BuildRelease"}) build.ArgumentList.Add(argument);
build.Environment["ACHIMELD_TRUST_CONFIG"] = trust;
var archive = new ProcessStartInfo("pwsh.exe") { WorkingDirectory=repository,UseShellExecute=false,CreateNoWindow=true };
foreach (string argument in new[] {"-NoProfile","-File",Path.Combine(repository,"build","Export-AuditSource.ps1"),"-OutputDirectory",Path.Combine(package,"audit")}) archive.ArgumentList.Add(argument);
await BuildProcessOutput.RunAsync(archive, "Archiving package source", "Source archive failed.");
await BuildProcessOutput.RunAsync(build, "Compiling the Rust desktop (a clean release build can take several minutes)", "Bridge release build failed.");
using (var index=JsonDocument.Parse(File.ReadAllBytes(Path.Combine(package,"audit","source-index.json")))) {
    foreach(var file in index.RootElement.GetProperty("files").EnumerateArray()) {
        string relative = file.GetProperty("path").GetString()!;
        if(Hash(Path.Combine(repository,relative))!=file.GetProperty("sha256").GetString())
            throw new InvalidOperationException($"Source changed during release build: '{relative}'. Wait for source edits to finish, then rerun the build.");
    }
    // Bind the previously compiled receiving assembly to the exact archived source.
    using var receiver = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(package,"read-worker-provenance.json")));
    if (receiver.RootElement.GetProperty("policy").GetString() != "reviewed-sync-v1" ||
        Hash(Path.Combine(package,"Achimeld.SteamWorker.dll")) != receiver.RootElement.GetProperty("workerAssemblySha256").GetString())
        throw new InvalidOperationException("Receiving assembly provenance mismatch.");
    foreach (var input in receiver.RootElement.GetProperty("source").EnumerateArray()) {
        string relative = input.GetProperty("path").GetString()!;
        var archived = index.RootElement.GetProperty("files").EnumerateArray().Single(file => file.GetProperty("path").GetString() == relative);
        if (archived.GetProperty("sha256").GetString() != input.GetProperty("sha256").GetString())
            throw new InvalidOperationException("Receiver source differs from packaged audit source.");
    }
    foreach (var input in receiver.RootElement.GetProperty("inputs").EnumerateArray()) {
        if (Hash(Path.Combine(package,input.GetProperty("path").GetString()!)) != input.GetProperty("sha256").GetString())
            throw new InvalidOperationException("Published worker runtime input changed during packaging.");
    }
    using var browserEvidence = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(package,"browser-provenance.json")));
    var browserLock = index.RootElement.GetProperty("files").EnumerateArray().Single(file => file.GetProperty("path").GetString() == "build/browser-runtime.lock.json");
    if (browserLock.GetProperty("sha256").GetString() != browserEvidence.RootElement.GetProperty("runtimeLockSha256").GetString())
        throw new InvalidOperationException("Browser lock differs from archived build source.");
    foreach (var input in browserEvidence.RootElement.GetProperty("files").EnumerateArray())
        if (Hash(Path.Combine(package,input.GetProperty("path").GetString()!)) != input.GetProperty("sha256").GetString())
            throw new InvalidOperationException("Pinned browser content changed during packaging.");
}
File.Copy(trust,Path.Combine(package,"audit","trusted-development.json"));
File.Copy(Path.Combine(target,"release","Achimeld.exe"),Path.Combine(package,"Achimeld.exe"));
File.Copy(Path.Combine(target,"release","achimeld-app.exe"),Path.Combine(package,"Achimeld.App.exe"));
File.Copy(Path.Combine(target,"release","achimeld-node-host.exe"),Path.Combine(package,"Achimeld.NodeHost.exe"));
var files = Directory.EnumerateFiles(package,"*",SearchOption.AllDirectories)
    .Select(path => new {path=Path.GetRelativePath(package,path).Replace('\\','/'),size=new FileInfo(path).Length,sha256=Hash(path)})
    .OrderBy(file=>file.path,StringComparer.Ordinal).ToArray();
string browser=files.Single(file=>file.path.StartsWith("browser/",StringComparison.Ordinal) && file.path.EndsWith("/chrome.exe",StringComparison.Ordinal)).path;
string[] allowedExecutables = ["Achimeld.exe", "Achimeld.App.exe", "Achimeld.NodeHost.exe", "Achimeld.SteamWorker.exe", ".playwright/node/win32_x64/node.exe", browser];
if (files.Any(file => file.path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase) && !allowedExecutables.Contains(file.path, StringComparer.Ordinal)))
    throw new InvalidOperationException("Unexpected executable in the read package.");
var programs=files.Where(file=>file.path.EndsWith(".exe",StringComparison.OrdinalIgnoreCase) && file.path!="Achimeld.exe")
    .Select(file=>new {file.path,role=file.path=="Achimeld.SteamWorker.exe" || file.path==browser || file.path.EndsWith("/node.exe",StringComparison.OrdinalIgnoreCase) ? "steamBroker":"offline"}).ToArray();
string sourceCommit=await Git("rev-parse","HEAD");
bool sourceDirty=(await Git("status","--porcelain")).Length!=0;
byte[] manifest=JsonSerializer.SerializeToUtf8Bytes(new {schemaVersion=1,product="Achimeld",platform="windows-x64",version="0.1.0",sourceCommit,sourceDirty,mode="development",keyId,createdUtc=DateTimeOffset.UtcNow.ToString("O"),files,
    network=new {proxyPort=42761,browserExecutable=browser,programs}});
File.WriteAllBytes(Path.Combine(package,"manifest.json"),manifest);
File.WriteAllText(Path.Combine(package,"manifest.sig"),Convert.ToBase64String(key.SignData(manifest,HashAlgorithmName.SHA256,RSASignaturePadding.Pss)));
var verify = new ProcessStartInfo(Path.Combine(package,"Achimeld.exe")) { WorkingDirectory=package,UseShellExecute=false,CreateNoWindow=true };
verify.ArgumentList.Add("--verify");
await BuildProcessOutput.RunAsync(verify, "Verifying the signed package", "Signed bridge verification failed.");
Console.WriteLine("Development bridge signed and verified; private signing key discarded.");
static string Hash(string path) { using var file=File.OpenRead(path);return Convert.ToHexStringLower(SHA256.HashData(file)); }
async Task<string> Git(params string[] arguments) {
    var start=new ProcessStartInfo("git") {WorkingDirectory=repository,UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true};
    foreach(string argument in arguments) start.ArgumentList.Add(argument);
    using var process=Process.Start(start) ?? throw new InvalidOperationException("Cannot inspect source state.");
    string result=await process.StandardOutput.ReadToEndAsync();await process.WaitForExitAsync();
    if(process.ExitCode!=0) throw new InvalidOperationException("Source state unavailable.");return result.Trim();
}
