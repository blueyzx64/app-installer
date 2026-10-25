//! PackageKit enum values as seen on the D-Bus wire.
//!
//! Source: `lib/packagekit-glib2/pk-enum.h` (PackageKit 1.2/1.3).
//! ⚠ Numeric values come from memory of that header and have NOT been checked
//! against the installed PackageKit. Verify on Fedora 45 (HUMAN) before release.
//! `Status`, `Exit`, `Error`, `Info` are plain u32 enum values on the wire.
//! `Filter` and `TransactionFlag` are u64 bitfields: bit = `1 << enum_value` (⚠).

macro_rules! pk_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $val:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($var = $val),+ }
        impl $name {
            /// Unknown numbers map to `Unknown` (PackageKit adds values over time).
            pub fn from_u32(v: u32) -> Self {
                match v { $($val => Self::$var,)+ _ => Self::Unknown }
            }
        }
    };
}

pk_enum!(
    /// `PkStatusEnum`: the transaction `Status` property.
    PkStatus {
        Unknown = 0, Wait = 1, Setup = 2, Run = 3, Query = 4, Info = 5, Remove = 6,
        RefreshCache = 7, Download = 8, Install = 9, Update = 10, Cleanup = 11,
        Obsolete = 12, DepResolve = 13, SigCheck = 14, TestCommit = 15, Commit = 16,
        Request = 17, Finished = 18, Cancel = 19, DownloadRepository = 20,
        DownloadPackagelist = 21, DownloadFilelist = 22, DownloadChangelog = 23,
        DownloadGroup = 24, DownloadUpdateinfo = 25, Repackaging = 26, LoadingCache = 27,
        ScanApplications = 28, GeneratePackageList = 29, WaitingForLock = 30,
        WaitingForAuth = 31, ScanProcessList = 32, CheckExecutableFiles = 33,
        CheckLibraries = 34, CopyFiles = 35, RunHook = 36,
    }
);

pk_enum!(
    /// `PkExitEnum`: the `Finished` signal.
    PkExit {
        Unknown = 0, Success = 1, Failed = 2, Cancelled = 3, KeyRequired = 4,
        EulaRequired = 5, Killed = 6, MediaChangeRequired = 7, NeedUntrusted = 8,
        CancelledPriority = 9, SkipTransaction = 10, RepairRequired = 11,
    }
);

pk_enum!(
    /// `PkInfoEnum`: the first field of the `Package` signal.
    PkInfo {
        Unknown = 0, Installed = 1, Available = 2, Low = 3, Enhancement = 4, Normal = 5,
        Bugfix = 6, Important = 7, Security = 8, Blocked = 9, Downloading = 10,
        Updating = 11, Installing = 12, Removing = 13, Cleanup = 14, Obsoleting = 15,
        CollectionInstalled = 16, CollectionAvailable = 17, Finished = 18,
        Reinstalling = 19, Downgrading = 20, Preparing = 21, Decompressing = 22,
        Untrusted = 23, Trusted = 24, Unavailable = 25, Critical = 26, Install = 27,
        Remove = 28, Obsolete = 29, Downgrade = 30,
    }
);

pk_enum!(
    /// `PkErrorEnum`: the `ErrorCode` signal.
    PkError {
        Unknown = 0, Oom = 1, NoNetwork = 2, NotSupported = 3, InternalError = 4,
        GpgFailure = 5, PackageIdInvalid = 6, PackageNotInstalled = 7,
        PackageNotFound = 8, PackageAlreadyInstalled = 9, PackageDownloadFailed = 10,
        GroupNotFound = 11, GroupListInvalid = 12, DepResolutionFailed = 13,
        FilterInvalid = 14, CreateThreadFailed = 15, TransactionError = 16,
        TransactionCancelled = 17, NoCache = 18, RepoNotFound = 19,
        CannotRemoveSystemPackage = 20, ProcessKill = 21, FailedInitialization = 22,
        FailedFinalise = 23, FailedConfigParsing = 24, CannotCancel = 25,
        CannotGetLock = 26, NoPackagesToUpdate = 27, CannotWriteRepoConfig = 28,
        LocalInstallFailed = 29, BadGpgSignature = 30, MissingGpgSignature = 31,
        CannotInstallSourcePackage = 32, RepoConfigurationError = 33,
        NoLicenseAgreement = 34, FileConflicts = 35, PackageConflicts = 36,
        RepoNotAvailable = 37, InvalidPackageFile = 38, PackageInstallBlocked = 39,
        PackageCorrupt = 40, AllPackagesAlreadyInstalled = 41, FileNotFound = 42,
        NoMoreMirrorsToTry = 43, NoDistroUpgradeData = 44, IncompatibleArchitecture = 45,
        NoSpaceOnDevice = 46, MediaChangeRequired = 47, NotAuthorized = 48,
        UpdateNotFound = 49, CannotInstallRepoUnsigned = 50,
        CannotUpdateRepoUnsigned = 51, CannotGetFilelist = 52, CannotGetRequests = 53,
        CannotDisableRepository = 54, RestrictedDownload = 55,
        PackageFailedToConfigure = 56, PackageFailedToBuild = 57,
        PackageFailedToInstall = 58, PackageFailedToRemove = 59,
        UpdateFailedDueToRunningProcess = 60, PackageDatabaseChanged = 61,
        ProvideTypeNotSupported = 62, InstallRootInvalid = 63, CannotFetchSources = 64,
        CancelledPriority = 65, UnfinishedTransaction = 66, LockRequired = 67,
    }
);

/// `PkFilterEnum` bit positions (⚠ bit = 1 << value).
pub mod filter {
    pub const NONE: u64 = 1 << 1;
    pub const INSTALLED: u64 = 1 << 2;
    pub const NOT_INSTALLED: u64 = 1 << 3;
}

/// `PkTransactionFlagEnum` bit positions (⚠ bit = 1 << value; NONE is value 0).
pub mod flag {
    pub const NONE: u64 = 1 << 0;
    pub const ONLY_TRUSTED: u64 = 1 << 1;
    pub const SIMULATE: u64 = 1 << 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_values_and_unknown_fallback() {
        assert_eq!(PkStatus::from_u32(31), PkStatus::WaitingForAuth);
        assert_eq!(PkStatus::from_u32(9999), PkStatus::Unknown);
        assert_eq!(PkExit::from_u32(3), PkExit::Cancelled);
        assert_eq!(PkError::from_u32(48), PkError::NotAuthorized);
        assert_eq!(PkError::from_u32(31), PkError::MissingGpgSignature);
        assert_eq!(PkInfo::from_u32(13), PkInfo::Removing);
        assert_eq!(PkInfo::from_u32(12), PkInfo::Installing);
    }

    #[test]
    fn flags_are_distinct_bits() {
        assert_eq!(flag::ONLY_TRUSTED & flag::SIMULATE, 0);
        assert_eq!(filter::INSTALLED & filter::NONE, 0);
    }
}
