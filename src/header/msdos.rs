//! MS-DOS attributes
use core::fmt;
use bitflags::bitflags;

bitflags! {
    #[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    /// MS-DOS attributes
    pub struct MsDosAttrs: u16 {
        const READ_ONLY = 0b00000000_00000001;
        const HIDDEN    = 0b00000000_00000010;
        const SYSTEM    = 0b00000000_00000100;
        const VOLUME    = 0b00000000_00001000;
        const SUBDIR    = 0b00000000_00010000;
        const ARCHIVE   = 0b00000000_00100000;
        const SYMLINK   = 0b00000000_01000000;
        const RESERVED  = 0b11111111_10000000;
    }
}

impl MsDosAttrs {
    /// Return whether a read-only flag is set.
    pub fn is_read_only(self) -> bool {
        self.intersects(MsDosAttrs::READ_ONLY)
    }
    /// Return whether a hidden flag is set.
    pub fn is_hidden(self) -> bool {
        self.intersects(MsDosAttrs::HIDDEN)
    }
    /// Return whether a system flag is set.
    pub fn is_system(self) -> bool {
        self.intersects(MsDosAttrs::SYSTEM)
    }
    /// Return whether a system flag is set.
    pub fn is_volume(self) -> bool {
        self.intersects(MsDosAttrs::VOLUME)
    }
    /// Return whether a system flag is set.
    pub fn is_subdir(self) -> bool {
        self.intersects(MsDosAttrs::SUBDIR)
    }
    /// Return whether an archive flag is set.
    pub fn is_archive(self) -> bool {
        self.intersects(MsDosAttrs::ARCHIVE)
    }
    /// Return whether a symlink flag is set.
    pub fn is_symlink(self) -> bool {
        self.intersects(MsDosAttrs::SYMLINK)
    }
}

impl fmt::Display for MsDosAttrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags: [u8;5] = *b"ADSHR";
        let mask: usize = const {
                    MsDosAttrs::READ_ONLY.
            r#union(MsDosAttrs::HIDDEN).
            r#union(MsDosAttrs::SYSTEM).
            r#union(MsDosAttrs::SUBDIR).
            r#union(MsDosAttrs::ARCHIVE)
            .bits()
        }.into();
        let mut bit: usize = MsDosAttrs::ARCHIVE.bits().into();
        let perm: usize = self.bits().into();
        for ch in flags.iter_mut() {
            if (mask & bit) == 0 {
                bit >>= 1;
            }
            if (perm & bit) == 0 {
                *ch = b'-';
            }
            bit >>= 1;
        }
        let flags = str::from_utf8(&flags).unwrap();
        flags.fmt(f)
    }
}


#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use super::*;

    #[test]
    fn msdos_attributes_works() {
        assert!(MsDosAttrs::READ_ONLY.is_read_only());
        assert!(MsDosAttrs::HIDDEN.is_hidden());
        assert!(MsDosAttrs::SYSTEM.is_system());
        assert!(MsDosAttrs::VOLUME.is_volume());
        assert!(MsDosAttrs::SUBDIR.is_subdir());
        assert!(MsDosAttrs::ARCHIVE.is_archive());
        assert!(MsDosAttrs::SYMLINK.is_symlink());
        assert!(!MsDosAttrs::empty().is_read_only());
        assert!(!MsDosAttrs::empty().is_hidden());
        assert!(!MsDosAttrs::empty().is_system());
        assert!(!MsDosAttrs::empty().is_volume());
        assert!(!MsDosAttrs::empty().is_subdir());
        assert!(!MsDosAttrs::empty().is_archive());
        assert!(!MsDosAttrs::empty().is_symlink());
        assert_eq!(MsDosAttrs::READ_ONLY.to_string(), "----R");
        assert_eq!(MsDosAttrs::HIDDEN.to_string(),    "---H-");
        assert_eq!(MsDosAttrs::SYSTEM.to_string(),    "--S--");
        assert_eq!(MsDosAttrs::VOLUME.to_string(),    "-----");
        assert_eq!(MsDosAttrs::SUBDIR.to_string(),    "-D---");
        assert_eq!(MsDosAttrs::ARCHIVE.to_string(),   "A----");
        assert_eq!(MsDosAttrs::SYMLINK.to_string(),   "-----");
        assert_eq!(MsDosAttrs::empty().to_string(),   "-----");
        assert_eq!(MsDosAttrs::all().to_string(),     "ADSHR");
        assert_eq!(MsDosAttrs::all().to_string(),     "ADSHR");
        assert_eq!((MsDosAttrs::READ_ONLY|
                    MsDosAttrs::HIDDEN|
                    MsDosAttrs::SYSTEM|
                    MsDosAttrs::SUBDIR|
                    MsDosAttrs::ARCHIVE).to_string(),   "ADSHR");
        assert_eq!((MsDosAttrs::READ_ONLY|
                    MsDosAttrs::ARCHIVE).to_string(),   "A---R");
        assert_eq!((MsDosAttrs::HIDDEN|
                    MsDosAttrs::SYSTEM|
                    MsDosAttrs::SUBDIR).to_string(),    "-DSH-");
    }
}
