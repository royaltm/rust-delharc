//! OS-9 specific data types
use core::fmt;
use bitflags::bitflags;
use super::Permissions;

bitflags! {
    #[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    /// OS-9 attributes
    pub struct Os9Attrs: u8 {
        /// File owner has a read permission
        const R_USER      = 0b00000001;
        /// File owner has a write permission
        const W_USER      = 0b00000010;
        /// File owner has an executable permission
        const E_USER      = 0b00000100;
        /// Mask of the user permission bits
        const PERM_USER   = 0b00000111;
        /// Other users have a read permission
        const R_OTHER     = 0b00001000;
        /// Other users have a write permission
        const W_OTHER     = 0b00010000;
        /// Other users have an executable permission
        const E_OTHER     = 0b00100000;
        /// Mask of the other permission bits
        const PERM_OTHER  = 0b00111000;
        /// Shareable (locked) bit.
        ///
        /// The file can only be open by a single process.
        const SHAREABLE   = 0b01000000;
        /// The entry is a directory
        const TYPE_DIR    = 0b10000000;
    }
}

impl Os9Attrs {
    /// Return whether these are a directory entry's attributes
    pub fn is_dir(self) -> bool {
        self.intersects(Os9Attrs::TYPE_DIR)
    }
    /// Return whether these are a file entry's attributes
    pub fn is_file(self) -> bool {
        !self.is_dir()
    }
    /// Return only the file owner permission flags
    pub fn user(self) -> Self {
        self.intersection(Os9Attrs::PERM_USER)
    }
    /// Return only the other permission flags
    pub fn other(self) -> Self {
        self.intersection(Os9Attrs::PERM_OTHER)
    }
    /// Return whether any executable bit is set.
    pub fn is_executable(self) -> bool {
        self.intersects(Os9Attrs::E_OTHER|Os9Attrs::E_USER)
    }
    /// Return whether any writable bit is set.
    pub fn is_writable(self) -> bool {
        self.intersects(Os9Attrs::W_OTHER|Os9Attrs::W_USER)
    }
    /// Return whether any readable bit is set.
    pub fn is_readable(self) -> bool {
        self.intersects(Os9Attrs::R_OTHER|Os9Attrs::R_USER)
    }
    /// Return whether a shareable flag is set.
    pub fn is_shareable(self) -> bool {
        self.intersects(Os9Attrs::SHAREABLE)
    }
}

impl fmt::Display for Os9Attrs {
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags: [u8;8] = *b"--ewrewr";
        if self.is_dir() {
            flags[0] = b'd';
        }
        if self.is_shareable() {
            flags[1] = b's';
        }
        let mut mask = Os9Attrs::PERM_OTHER;
        for i in (2..=5).step_by(3) {
            let perm = self.intersection(mask);
            if !perm.is_executable() {
                flags[i] = b'-';
            }
            if !perm.is_writable() {
                flags[i + 1] = b'-';
            }
            if !perm.is_readable() {
                flags[i + 2] = b'-';
            }
            mask = Os9Attrs::from_bits_retain(mask.bits() >> 3);
        }
        let flags = str::from_utf8(&flags).unwrap();
        flags.fmt(f)
    }
}

impl From<Os9Attrs> for Permissions {
    fn from(attr: Os9Attrs) -> Permissions {
        let mut perm = Permissions::empty();
        let user = attr.user();
        perm.set(Permissions::R_USER, user.is_readable());
        perm.set(Permissions::W_USER, user.is_writable());
        perm.set(Permissions::X_USER, user.is_executable());
        let other = attr.other();
        perm.set(Permissions::R_OTHER|Permissions::R_GROUP, other.is_readable());
        perm.set(Permissions::W_OTHER|Permissions::W_GROUP, other.is_writable());
        perm.set(Permissions::X_OTHER|Permissions::X_GROUP, other.is_executable());
        perm.insert(if attr.is_dir() {
            Permissions::TYPE_DIR
        }
        else {
            Permissions::TYPE_FILE
        });
        perm
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::string::ToString;
    use super::*;

    #[test]
    fn os_9_attributes_works() {
        assert!(Os9Attrs::E_OTHER.is_executable());
        assert!(Os9Attrs::W_OTHER.is_writable());
        assert!(Os9Attrs::R_OTHER.is_readable());
        assert!(Os9Attrs::E_OTHER.other().is_executable());
        assert!(Os9Attrs::W_OTHER.other().is_writable());
        assert!(Os9Attrs::R_OTHER.other().is_readable());
        assert!(!Os9Attrs::E_OTHER.user().is_executable());
        assert!(!Os9Attrs::W_OTHER.user().is_writable());
        assert!(!Os9Attrs::R_OTHER.user().is_readable());
        assert!(Os9Attrs::PERM_OTHER.is_executable());
        assert!(Os9Attrs::PERM_OTHER.is_writable());
        assert!(Os9Attrs::PERM_OTHER.is_readable());
        assert!(Os9Attrs::PERM_OTHER.other().is_executable());
        assert!(Os9Attrs::PERM_OTHER.other().is_writable());
        assert!(Os9Attrs::PERM_OTHER.other().is_readable());
        assert!(!Os9Attrs::PERM_OTHER.user().is_executable());
        assert!(!Os9Attrs::PERM_OTHER.user().is_writable());
        assert!(!Os9Attrs::PERM_OTHER.user().is_readable());
        assert!(Os9Attrs::E_USER.is_executable());
        assert!(Os9Attrs::W_USER.is_writable());
        assert!(Os9Attrs::R_USER.is_readable());
        assert!(Os9Attrs::E_USER.user().is_executable());
        assert!(Os9Attrs::W_USER.user().is_writable());
        assert!(Os9Attrs::R_USER.user().is_readable());
        assert!(!Os9Attrs::E_USER.other().is_executable());
        assert!(!Os9Attrs::W_USER.other().is_writable());
        assert!(!Os9Attrs::R_USER.other().is_readable());
        assert!(Os9Attrs::PERM_USER.is_executable());
        assert!(Os9Attrs::PERM_USER.is_writable());
        assert!(Os9Attrs::PERM_USER.is_readable());
        assert!(Os9Attrs::PERM_USER.user().is_executable());
        assert!(Os9Attrs::PERM_USER.user().is_writable());
        assert!(Os9Attrs::PERM_USER.user().is_readable());
        assert!(!Os9Attrs::PERM_USER.other().is_executable());
        assert!(!Os9Attrs::PERM_USER.other().is_writable());
        assert!(!Os9Attrs::PERM_USER.other().is_readable());
        assert!(Os9Attrs::SHAREABLE.is_shareable());
        assert!(Os9Attrs::TYPE_DIR.is_dir());
        assert!(!Os9Attrs::TYPE_DIR.is_file());
        assert!(!Os9Attrs::empty().is_executable());
        assert!(!Os9Attrs::empty().is_writable());
        assert!(!Os9Attrs::empty().is_readable());
        assert!(!Os9Attrs::empty().other().is_executable());
        assert!(!Os9Attrs::empty().other().is_writable());
        assert!(!Os9Attrs::empty().other().is_readable());
        assert!(!Os9Attrs::empty().user().is_executable());
        assert!(!Os9Attrs::empty().user().is_writable());
        assert!(!Os9Attrs::empty().user().is_readable());
        assert!(!Os9Attrs::empty().is_shareable());
        assert!(!Os9Attrs::empty().is_dir());
        assert!(Os9Attrs::empty().is_file());
        assert_eq!(Os9Attrs::empty().to_string(),     "--------");
        assert_eq!(Os9Attrs::R_USER.to_string(),      "-------r");
        assert_eq!(Os9Attrs::W_USER.to_string(),      "------w-");
        assert_eq!(Os9Attrs::E_USER.to_string(),      "-----e--");
        assert_eq!(Os9Attrs::PERM_USER.to_string(),   "-----ewr");
        assert_eq!(Os9Attrs::R_OTHER.to_string(),     "----r---");
        assert_eq!(Os9Attrs::W_OTHER.to_string(),     "---w----");
        assert_eq!(Os9Attrs::E_OTHER.to_string(),     "--e-----");
        assert_eq!(Os9Attrs::PERM_OTHER.to_string(),  "--ewr---");
        assert_eq!(Os9Attrs::SHAREABLE.to_string(),   "-s------");
        assert_eq!(Os9Attrs::TYPE_DIR.to_string(),    "d-------");
        assert_eq!((Os9Attrs::TYPE_DIR|
                    Os9Attrs::SHAREABLE|
                    Os9Attrs::PERM_OTHER|
                    Os9Attrs::PERM_USER).to_string(), "dsewrewr");
        assert_eq!((Os9Attrs::SHAREABLE|
                    Os9Attrs::PERM_OTHER|
                    Os9Attrs::PERM_USER).to_string(), "-sewrewr");
        assert_eq!((Os9Attrs::TYPE_DIR|
                    Os9Attrs::PERM_OTHER|
                    Os9Attrs::PERM_USER).to_string(), "d-ewrewr");
        assert_eq!((Os9Attrs::TYPE_DIR|
                    Os9Attrs::SHAREABLE|
                    Os9Attrs::E_OTHER|
                    Os9Attrs::E_USER).to_string(),    "dse--e--");
        assert_eq!((Os9Attrs::SHAREABLE|
                    Os9Attrs::R_OTHER|
                    Os9Attrs::R_USER).to_string(),    "-s--r--r");
        assert_eq!((Os9Attrs::SHAREABLE|
                    Os9Attrs::W_OTHER|
                    Os9Attrs::W_USER).to_string(),    "-s-w--w-");
        assert_eq!((Os9Attrs::R_USER|
                    Os9Attrs::W_USER).to_string(),    "------wr");
        assert_eq!((Os9Attrs::R_OTHER|
                    Os9Attrs::W_OTHER).to_string(),   "---wr---");
        assert_eq!(Os9Attrs::all().to_string(),       "dsewrewr");

        for &(expected, perm) in &[
            ("drwxrwxrwx", Os9Attrs::TYPE_DIR|Os9Attrs::PERM_OTHER|Os9Attrs::PERM_USER),
            ("drwxrwxrwx", Os9Attrs::all()),
            ("----------", Os9Attrs::empty()),
            ("----------", Os9Attrs::SHAREABLE),
            ("-rwx------", Os9Attrs::PERM_USER),
            ("-r--------", Os9Attrs::R_USER),
            ("--w-------", Os9Attrs::W_USER),
            ("---x------", Os9Attrs::E_USER),
            ("----rwxrwx", Os9Attrs::PERM_OTHER),
            ("----r--r--", Os9Attrs::R_OTHER),
            ("-----w--w-", Os9Attrs::W_OTHER),
            ("------x--x", Os9Attrs::E_OTHER),
            ("drw-r--r--", Os9Attrs::TYPE_DIR|Os9Attrs::R_OTHER|Os9Attrs::W_USER|Os9Attrs::R_USER),
            ("-rwxr-xr-x", Os9Attrs::SHAREABLE|Os9Attrs::PERM_USER|Os9Attrs::R_OTHER|Os9Attrs::E_OTHER),
            ("drwxr-xr-x", Os9Attrs::TYPE_DIR|Os9Attrs::SHAREABLE|Os9Attrs::PERM_USER|Os9Attrs::R_OTHER|Os9Attrs::E_OTHER),
            ("-rwxr-xr-x", Os9Attrs::PERM_USER|Os9Attrs::R_OTHER|Os9Attrs::E_OTHER)]
        {
            assert_eq!(Permissions::from(perm).to_string(), expected);
        }
    }
}
