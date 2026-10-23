//! Persistent ALE policy. A soft, higher-weight permit is confined to the broker;
//! the lower-weight hard block covers every other destination/protocol.
use crate::manifest::{ProgramRole, VerifiedPackage};
use sha2::{Digest, Sha256};
use std::{ffi::c_void, ptr};
use windows_sys::{
    Win32::{Foundation::HANDLE, NetworkManagement::WindowsFilteringPlatform::*},
    core::GUID,
};

const PROVIDER: GUID = GUID::from_u128(0xc6779670_d475_49ab_91bc_6c549e230101);
const SUBLAYER: GUID = GUID::from_u128(0xc6779670_d475_49ab_91bc_6c549e230103);
// Denial must be evaluated before the built-in firewall's hard loopback permits.
// BFE normalizes 0xffff to the highest assignable priority, 0xfffe.
const SUBLAYER_WEIGHT: u16 = 0xfffe;
pub struct Engine(HANDLE);
impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: Engine exclusively owns a successful BFE engine handle and closes it once.
        unsafe {
            FwpmEngineClose0(self.0);
        }
    }
}
fn check(code: u32) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Windows network policy unavailable (0x{code:08x})"))
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}
fn equal(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}
fn key(package: &VerifiedPackage, path: &str, ipv6: bool, permit: bool) -> GUID {
    use std::os::windows::ffi::OsStrExt;
    let mut digest = Sha256::new();
    for unit in package.root().as_os_str().encode_wide() {
        let folded = if (b'A' as u16..=b'Z' as u16).contains(&unit) {
            unit + 32
        } else {
            unit
        };
        digest.update(folded.to_le_bytes());
    }
    digest.update([0, 0]);
    digest.update(format!("{}|{ipv6}|{permit}", path.to_ascii_lowercase()).as_bytes());
    let digest = digest.finalize();
    GUID::from_u128(u128::from_be_bytes(digest[..16].try_into().unwrap()))
}
impl Engine {
    pub fn open() -> Result<Self, String> {
        let mut handle = ptr::null_mut();
        // SAFETY: Null optional arguments and live output request a local engine; successful handle transfers to Engine.
        unsafe {
            check(FwpmEngineOpen0(
                ptr::null(),
                10,
                ptr::null(),
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Self(handle))
    }
    fn visit(
        &self,
        package: &VerifiedPackage,
        mut action: impl FnMut(&FWPM_FILTER0) -> Result<(), String>,
    ) -> Result<(), String> {
        for program in &package.manifest().network.programs {
            let filename = achimeld_security::windows_arguments::terminated(
                package.root().join(&program.path).as_os_str(),
            )
            .map_err(|e| e.to_string())?;
            let mut app_id = ptr::null_mut();
            // SAFETY: Terminated verified filename and app-ID output live through the call; BFE allocates the returned blob.
            unsafe {
                check(FwpmGetAppIdFromFileName0(filename.as_ptr(), &mut app_id))?;
            }
            let result: Result<(), String> = (|| {
                for ipv6 in [false, true] {
                    for permit in [false, true] {
                        if permit && (ipv6 || program.role != ProgramRole::SteamBroker) {
                            continue;
                        }
                        let mut conditions = vec![FWPM_FILTER_CONDITION0 {
                            fieldKey: FWPM_CONDITION_ALE_APP_ID,
                            matchType: FWP_MATCH_EQUAL,
                            conditionValue: FWP_CONDITION_VALUE0 {
                                r#type: FWP_BYTE_BLOB_TYPE,
                                Anonymous: FWP_CONDITION_VALUE0_0 { byteBlob: app_id },
                            },
                        }];
                        if permit {
                            conditions.extend([
                                FWPM_FILTER_CONDITION0 {
                                    fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
                                    matchType: FWP_MATCH_EQUAL,
                                    conditionValue: FWP_CONDITION_VALUE0 {
                                        r#type: FWP_UINT32,
                                        Anonymous: FWP_CONDITION_VALUE0_0 { uint32: 0x7f000001 },
                                    },
                                },
                                FWPM_FILTER_CONDITION0 {
                                    fieldKey: FWPM_CONDITION_IP_REMOTE_PORT,
                                    matchType: FWP_MATCH_EQUAL,
                                    conditionValue: FWP_CONDITION_VALUE0 {
                                        r#type: FWP_UINT16,
                                        Anonymous: FWP_CONDITION_VALUE0_0 {
                                            uint16: package.manifest().network.proxy_port,
                                        },
                                    },
                                },
                                FWPM_FILTER_CONDITION0 {
                                    fieldKey: FWPM_CONDITION_IP_PROTOCOL,
                                    matchType: FWP_MATCH_EQUAL,
                                    conditionValue: FWP_CONDITION_VALUE0 {
                                        r#type: FWP_UINT8,
                                        Anonymous: FWP_CONDITION_VALUE0_0 { uint8: 6 },
                                    },
                                },
                            ]);
                        }
                        let mut provider = PROVIDER;
                        let mut weight = if permit { 200_u64 } else { 100_u64 };
                        let mut label = wide("Achimeld persistent outbound isolation");
                        let filter = FWPM_FILTER0 {
                            filterKey: key(package, &program.path, ipv6, permit),
                            displayData: FWPM_DISPLAY_DATA0 {
                                name: label.as_mut_ptr(),
                                description: ptr::null_mut(),
                            },
                            flags: FWPM_FILTER_FLAG_PERSISTENT,
                            providerKey: &mut provider,
                            layerKey: if ipv6 {
                                FWPM_LAYER_ALE_AUTH_CONNECT_V6
                            } else {
                                FWPM_LAYER_ALE_AUTH_CONNECT_V4
                            },
                            subLayerKey: SUBLAYER,
                            weight: FWP_VALUE0 {
                                r#type: FWP_UINT64,
                                Anonymous: FWP_VALUE0_0 {
                                    uint64: &mut weight,
                                },
                            },
                            numFilterConditions: conditions.len() as u32,
                            filterCondition: conditions.as_mut_ptr(),
                            action: FWPM_ACTION0 {
                                r#type: if permit {
                                    FWP_ACTION_PERMIT
                                } else {
                                    FWP_ACTION_BLOCK
                                },
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        action(&filter)?;
                    }
                }
                Ok(())
            })();
            // SAFETY: app_id is the BFE allocation returned above; its owner frees it only after all borrowed filters are consumed.
            unsafe {
                FwpmFreeMemory0(&mut app_id as *mut _ as *mut *mut c_void);
            }
            result?;
        }
        Ok(())
    }
    pub fn install(&self, package: &VerifiedPackage) -> Result<(), String> {
        // Ordinary supervisors need read access, while only SYSTEM/Administrators
        // may alter the persistent objects. Pin the owner to Administrators too.
        let permissions =
            crate::platform::descriptor("O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GR;;;BU)")?;
        // SAFETY: Owned engine stays live through the transaction; subsequent paths commit or abort it.
        unsafe {
            check(FwpmTransactionBegin0(self.0, 0))?;
        }
        let result = (|| {
            let mut label = wide("Achimeld");
            let provider = FWPM_PROVIDER0 {
                providerKey: PROVIDER,
                displayData: FWPM_DISPLAY_DATA0 {
                    name: label.as_mut_ptr(),
                    ..Default::default()
                },
                flags: FWPM_PROVIDER_FLAG_PERSISTENT,
                ..Default::default()
            };
            let mut provider_key = PROVIDER;
            let sublayer = FWPM_SUBLAYER0 {
                subLayerKey: SUBLAYER,
                displayData: FWPM_DISPLAY_DATA0 {
                    name: label.as_mut_ptr(),
                    ..Default::default()
                },
                flags: FWPM_SUBLAYER_FLAG_PERSISTENT,
                providerKey: &mut provider_key,
                weight: SUBLAYER_WEIGHT,
                ..Default::default()
            };
            // SAFETY: Provider, sublayer labels and owned security descriptor remain live for synchronous BFE calls.
            for code in unsafe {
                [
                    FwpmProviderAdd0(self.0, &provider, permissions.0 as *mut c_void),
                    FwpmSubLayerAdd0(self.0, &sublayer, permissions.0 as *mut c_void),
                ]
            } {
                if code != 0x80320009 {
                    check(code)?;
                } // FWP_E_ALREADY_EXISTS
            }
            // SAFETY: visit keeps filter, provider, conditions, weight and app-ID storage live for synchronous delete/add calls.
            self.visit(package, |filter| unsafe {
                let code = FwpmFilterDeleteByKey0(self.0, &filter.filterKey);
                if code != 0x80320003 {
                    check(code)?;
                } // FWP_E_FILTER_NOT_FOUND
                check(FwpmFilterAdd0(
                    self.0,
                    filter,
                    permissions.0 as *mut c_void,
                    ptr::null_mut(),
                ))
            })
        })();
        // SAFETY: Owned engine has an active transaction; success commits it and failure aborts it.
        unsafe {
            if result.is_ok() {
                check(FwpmTransactionCommit0(self.0))?;
            } else {
                FwpmTransactionAbort0(self.0);
            }
        }
        result?;
        // Upgrade the shared objects too when repairing an older development policy.
        // SAFETY: Returned LocalAlloc security descriptor and nested SIDs stay live through the access checks and are freed once afterwards.
        unsafe {
            use windows_sys::Win32::Security::*;
            let mut owner = ptr::null_mut();
            let mut dacl = ptr::null_mut();
            let mut present = 0;
            let mut defaulted = 0;
            if GetSecurityDescriptorOwner(permissions.0 as *mut c_void, &mut owner, &mut defaulted)
                == 0
                || GetSecurityDescriptorDacl(
                    permissions.0 as *mut c_void,
                    &mut present,
                    &mut dacl,
                    &mut defaulted,
                ) == 0
                || present == 0
            {
                return Err("policy permissions unavailable".into());
            }
            // WFP accepts OWNER/DACL here but rejects the file-system-only
            // PROTECTED_DACL flag with FWP_E_INVALID_FLAGS.
            let information = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
            check(FwpmProviderSetSecurityInfoByKey0(
                self.0,
                &PROVIDER,
                information,
                owner as *const SID,
                ptr::null(),
                dacl,
                ptr::null(),
            ))?;
            check(FwpmSubLayerSetSecurityInfoByKey0(
                self.0,
                &SUBLAYER,
                information,
                owner as *const SID,
                ptr::null(),
                dacl,
                ptr::null(),
            ))?;
        }
        self.verify(package)
    }
    pub fn remove(&self, package: &VerifiedPackage) -> Result<(), String> {
        // SAFETY: Owned engine stays live through the removal transaction.
        unsafe {
            check(FwpmTransactionBegin0(self.0, 0))?;
        }
        // SAFETY: visit keeps the filter key live for the synchronous delete; no borrowed allocation escapes.
        let result = self.visit(package, |filter| unsafe {
            let code = FwpmFilterDeleteByKey0(self.0, &filter.filterKey);
            if code == 0x80320003 {
                Ok(())
            } else {
                check(code)
            }
        });
        // SAFETY: Owned engine has an active transaction; success commits it and failure aborts it.
        unsafe {
            if result.is_ok() {
                check(FwpmTransactionCommit0(self.0))?;
            } else {
                FwpmTransactionAbort0(self.0);
            }
        }
        result
    }
    pub fn verify(&self, package: &VerifiedPackage) -> Result<(), String> {
        // SAFETY: Successful BFE queries return owned provider/sublayer allocations; nested pointers are inspected before freeing them.
        unsafe {
            let mut provider = ptr::null_mut();
            check(FwpmProviderGetByKey0(self.0, &PROVIDER, &mut provider))?;
            let valid = (*provider).flags == FWPM_PROVIDER_FLAG_PERSISTENT;
            FwpmFreeMemory0(&mut provider as *mut _ as *mut *mut c_void);
            if !valid {
                return Err("Windows policy provider is disabled or altered".into());
            }
            let mut sublayer = ptr::null_mut();
            check(FwpmSubLayerGetByKey0(self.0, &SUBLAYER, &mut sublayer))?;
            let valid = (*sublayer).flags == FWPM_SUBLAYER_FLAG_PERSISTENT
                && (*sublayer).weight == SUBLAYER_WEIGHT
                && !(*sublayer).providerKey.is_null()
                && equal(&*(*sublayer).providerKey, &PROVIDER);
            let detail = format!(
                "flags {}, weight {} expected {}",
                (*sublayer).flags,
                (*sublayer).weight,
                SUBLAYER_WEIGHT
            );
            FwpmFreeMemory0(&mut sublayer as *mut _ as *mut *mut c_void);
            if !valid {
                return Err(format!("Windows policy sublayer was altered: {detail}"));
            }
        }
        // SAFETY: Actual BFE filter allocation and visit borrowed storage stay live through comparison and are freed after it; rejected pointers are not used for diagnostics.
        self.visit(package, |expected| unsafe {
            let mut actual = ptr::null_mut();
            check(FwpmFilterGetByKey0(
                self.0,
                &expected.filterKey,
                &mut actual,
            ))?;
            let valid = same_filter(&*actual, expected);
            FwpmFreeMemory0(&mut actual as *mut _ as *mut *mut c_void);
            if valid {
                Ok(())
            } else {
                Err("Windows network policy was altered".into())
            }
        })
    }
}
/// # Safety
/// Actual and its non-null nested pointers describe live native objects/allocated
/// memory of the reported sizes (normally a BFE allocation). Expected's borrowed
/// provider/weight/condition/app-ID storage remains live, as established by visit().
unsafe fn same_filter(actual: &FWPM_FILTER0, expected: &FWPM_FILTER0) -> bool {
    // SAFETY: Caller supplies live BFE/visit allocations as documented; every optional pointer/discriminant is checked before nested access.
    unsafe {
        // BFE adds INDEXED automatically; it is an optimization, not a policy permission.
        if actual.flags & !FWPM_FILTER_FLAG_INDEXED != expected.flags
            || actual.providerKey.is_null()
            || !equal(&*actual.providerKey, &PROVIDER)
            || !equal(&actual.layerKey, &expected.layerKey)
            || !equal(&actual.subLayerKey, &SUBLAYER)
            || actual.weight.r#type != FWP_UINT64
            || actual.weight.Anonymous.uint64.is_null()
            || *actual.weight.Anonymous.uint64 != *expected.weight.Anonymous.uint64
            || actual.action.r#type != expected.action.r#type
            || actual.numFilterConditions != expected.numFilterConditions
            || actual.filterCondition.is_null()
        {
            return false;
        }
        let conditions =
            std::slice::from_raw_parts(actual.filterCondition, actual.numFilterConditions as usize);
        for expected in std::slice::from_raw_parts(
            expected.filterCondition,
            expected.numFilterConditions as usize,
        ) {
            let Some(actual) = conditions
                .iter()
                .find(|c| equal(&c.fieldKey, &expected.fieldKey))
            else {
                return false;
            };
            if actual.matchType != FWP_MATCH_EQUAL
                || actual.conditionValue.r#type != expected.conditionValue.r#type
            {
                return false;
            }
            let a = actual.conditionValue.Anonymous;
            let b = expected.conditionValue.Anonymous;
            let matches = match actual.conditionValue.r#type {
                FWP_UINT8 => a.uint8 == b.uint8,
                FWP_UINT16 => a.uint16 == b.uint16,
                FWP_UINT32 => a.uint32 == b.uint32,
                FWP_BYTE_BLOB_TYPE => {
                    !a.byteBlob.is_null()
                        && !b.byteBlob.is_null()
                        && (*a.byteBlob).size > 0
                        && (*a.byteBlob).size == (*b.byteBlob).size
                        && (*a.byteBlob).size <= 65536
                        && !(*a.byteBlob).data.is_null()
                        && !(*b.byteBlob).data.is_null()
                        && std::slice::from_raw_parts(
                            (*a.byteBlob).data,
                            (*a.byteBlob).size as usize,
                        ) == std::slice::from_raw_parts(
                            (*b.byteBlob).data,
                            (*b.byteBlob).size as usize,
                        )
                }
                _ => false,
            };
            if !matches {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_filter_pointers_are_rejected_without_diagnostic_dereferences() {
        let mut provider = PROVIDER;
        let mut weight = 100u64;
        let mut bytes = [1u8];
        let mut blob = FWP_BYTE_BLOB {
            size: 1,
            data: bytes.as_mut_ptr(),
        };
        let mut condition = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_ALE_APP_ID,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0 {
                r#type: FWP_BYTE_BLOB_TYPE,
                Anonymous: FWP_CONDITION_VALUE0_0 {
                    byteBlob: &mut blob,
                },
            },
        };
        let expected = FWPM_FILTER0 {
            providerKey: &mut provider,
            subLayerKey: SUBLAYER,
            weight: FWP_VALUE0 {
                r#type: FWP_UINT64,
                Anonymous: FWP_VALUE0_0 {
                    uint64: &mut weight,
                },
            },
            numFilterConditions: 1,
            filterCondition: &mut condition,
            action: FWPM_ACTION0 {
                r#type: FWP_ACTION_BLOCK,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: Every native pointer references the typed live local storage above.
        assert!(unsafe { same_filter(&expected, &expected) });
        let mut actual = expected;
        actual.filterCondition = ptr::null_mut();
        // SAFETY: Null optional pointer is rejected before dereference; all other storage is live.
        assert!(!unsafe { same_filter(&actual, &expected) });
        let mut null_blob = FWP_BYTE_BLOB {
            size: 1,
            data: ptr::null_mut(),
        };
        let mut invalid_condition = condition;
        invalid_condition.conditionValue.Anonymous.byteBlob = &mut null_blob;
        actual.filterCondition = &mut invalid_condition;
        // SAFETY: Native blob object is live; its null data pointer is rejected before slicing.
        assert!(!unsafe { same_filter(&actual, &expected) });
    }
}
