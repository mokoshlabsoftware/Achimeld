//! Editor launch recipe: no inheritance, suspended launch, Job assignment before resume.
use super::handles::{Handle, error};
use achimeld_security::windows_arguments::{command_line, terminated};
use std::{ffi::c_void, path::Path, ptr};
use windows_sys::Win32::System::{JobObjects::*, Threading::*};

pub struct Job(pub Handle);
impl Job {
    pub fn create() -> Result<Self, String> {
        // SAFETY: Null name/security request an unnamed non-inheritable Job; successful handle is checked and owned.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(error("cannot create supervision job"));
        }
        let job = Self(Handle(raw as usize));
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 256;
        // SAFETY: Owned Job and initialized limits storage are live with the correct struct size.
        unsafe {
            if SetInformationJobObject(
                job.0.raw(),
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const c_void,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                return Err(error("cannot enforce job lifetime"));
            }
        }
        Ok(job)
    }
    pub fn contains(&self, pid: u32) -> bool {
        // SAFETY: PID is only used for an inspection handle, checked before creating its owner.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if raw.is_null() {
            return false;
        }
        let process = Handle(raw as usize);
        let mut member = 0;
        // SAFETY: Both owned handles and membership output are live for the synchronous query.
        unsafe { IsProcessInJob(process.raw(), self.0.raw(), &mut member) != 0 && member != 0 }
    }
    pub fn pids(&self) -> Result<Vec<u32>, String> {
        let mut buffer = vec![0usize; 1024];
        // SAFETY: Aligned initialized storage exceeds the capped Job PID-list capacity and remains live during the query.
        unsafe {
            if QueryInformationJobObject(
                self.0.raw(),
                JobObjectBasicProcessIdList,
                buffer.as_mut_ptr() as *mut c_void,
                (buffer.len() * size_of::<usize>()) as u32,
                ptr::null_mut(),
            ) == 0
            {
                return Err(error("process attribution unavailable"));
            }
        }
        // SAFETY: The successful native query filled aligned storage at least as large as the fixed list header.
        let value = unsafe { &*(buffer.as_ptr() as *const JOBOBJECT_BASIC_PROCESS_ID_LIST) };
        if value.NumberOfAssignedProcesses != value.NumberOfProcessIdsInList
            || value.NumberOfProcessIdsInList > 256
        {
            return Err("process attribution incomplete".into());
        }
        // SAFETY: Reported count is capped at 256 and fits the initialized backing storage; the slice is copied before it expires.
        Ok(unsafe {
            std::slice::from_raw_parts(
                value.ProcessIdList.as_ptr(),
                value.NumberOfProcessIdsInList as usize,
            )
        }
        .iter()
        .map(|pid| *pid as u32)
        .collect())
    }
    pub fn terminate(&self) {
        // SAFETY: Job handle remains owned during termination; this does not transfer or close it.
        unsafe {
            TerminateJobObject(self.0.raw(), 125);
        }
    }
    pub fn launch(
        &self,
        root: &Path,
        pipe: &str,
        arguments: &[&std::ffi::OsStr],
    ) -> Result<Handle, String> {
        let exe = root.join("Achimeld.App.exe");
        let mut command = command_line(exe.as_os_str(), arguments).map_err(|e| e.to_string())?;
        let exe = terminated(exe.as_os_str()).map_err(|e| e.to_string())?;
        let mut environment: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os()
            .filter(|(name, _)| {
                matches!(
                    name.to_str().unwrap_or("").to_ascii_uppercase().as_str(),
                    "SYSTEMROOT"
                        | "WINDIR"
                        | "LOCALAPPDATA"
                        | "APPDATA"
                        | "TEMP"
                        | "TMP"
                        | "USERPROFILE"
                        | "HOMEDRIVE"
                        | "HOMEPATH"
                        | "USERNAME"
                        | "USERDOMAIN"
                        | "PROGRAMDATA"
                        | "PROGRAMFILES"
                        | "PROGRAMFILES(X86)"
                )
            })
            .collect();
        environment.extend([
            ("ACHIMELD_SUPERVISION_PIPE".into(), pipe.into()),
            ("ACHIMELD_PACKAGE_ROOT".into(), root.as_os_str().to_owned()),
            ("PATH".into(), {
                let mut path = root.as_os_str().to_owned();
                path.push(";");
                path.push(
                    std::path::PathBuf::from(
                        std::env::var_os("SystemRoot").ok_or("Windows directory missing")?,
                    )
                    .join("System32"),
                );
                path
            }),
            (
                "PLAYWRIGHT_BROWSERS_PATH".into(),
                root.join("browser").into_os_string(),
            ),
        ]);
        environment.sort_by_key(|(key, _)| key.to_ascii_uppercase());
        let mut block = Vec::new();
        for (key, value) in environment {
            let mut entry = key;
            entry.push("=");
            entry.push(value);
            block.extend(terminated(&entry).map_err(|e| e.to_string())?);
        }
        block.push(0);
        let directory = terminated(root.as_os_str()).map_err(|e| e.to_string())?;
        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        // SAFETY: All launch buffers are live; child is suspended, inherits no handles and joins this Job before resume; outputs gain exclusive owners.
        unsafe {
            if CreateProcessW(
                exe.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                // No console association: CREATE_NO_WINDOW still creates conhost on
                // the supported Windows build. This does not break away from any Job.
                CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | DETACHED_PROCESS,
                block.as_ptr() as *const c_void,
                directory.as_ptr(),
                &startup,
                &mut info,
            ) == 0
            {
                return Err(error("cannot start verified editor"));
            }
            let process = Handle(info.hProcess as usize);
            let thread = Handle(info.hThread as usize);
            if AssignProcessToJobObject(self.0.raw(), process.raw()) == 0 {
                TerminateProcess(process.raw(), 125);
                return Err(error("cannot supervise editor"));
            }
            if ResumeThread(thread.raw()) == u32::MAX {
                self.terminate();
                return Err(error("cannot resume supervised editor"));
            }
            Ok(process)
        }
    }
}
