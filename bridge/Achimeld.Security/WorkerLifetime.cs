using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace Achimeld.Security;

public sealed class WorkerLifetime : IDisposable
{
    private readonly SafeFileHandle _job;
    public WorkerLifetime()
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException();
        _job = Native.CreateJobObjectW(nint.Zero,nint.Zero);
        if (_job.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
        ExtendedLimits limits = new() { Basic = new BasicLimits { Flags = 0x2000 /* KILL_ON_JOB_CLOSE */ } };
        if (!Native.SetInformationJobObject(_job,9,ref limits,Marshal.SizeOf<ExtendedLimits>()) ||
            !Native.AssignProcessToJobObject(_job,Native.GetCurrentProcess()))
        {
            int error=Marshal.GetLastWin32Error();_job.Dispose();throw new Win32Exception(error);
        }
    }
    public void Dispose() => _job.Dispose();

    [StructLayout(LayoutKind.Sequential)] private struct BasicLimits
    {
        public long ProcessTime,JobTime;
        public uint Flags;
        public nuint MinimumWorkingSet,MaximumWorkingSet;
        public uint ActiveProcesses;
        public nuint Affinity;
        public uint Priority,Scheduling;
    }
    [StructLayout(LayoutKind.Sequential)] private struct IoCounters
    {
        public ulong ReadOperations,WriteOperations,OtherOperations,ReadBytes,WriteBytes,OtherBytes;
    }
    [StructLayout(LayoutKind.Sequential)] private struct ExtendedLimits
    {
        public BasicLimits Basic;
        public IoCounters Io;
        public nuint ProcessMemory,JobMemory,PeakProcessMemory,PeakJobMemory;
    }
    private static class Native
    {
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
        public static extern SafeFileHandle CreateJobObjectW(nint attributes,nint name);
        [DllImport("kernel32.dll",SetLastError=true)] [return:MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetInformationJobObject(SafeFileHandle job,int informationClass,ref ExtendedLimits limits,int length);
        [DllImport("kernel32.dll",SetLastError=true)] [return:MarshalAs(UnmanagedType.Bool)]
        public static extern bool AssignProcessToJobObject(SafeFileHandle job,nint process);
        [DllImport("kernel32.dll")]
        public static extern nint GetCurrentProcess();
    }
}
