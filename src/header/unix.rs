//! UNIX permissions
use core::fmt;
use bitflags::bitflags;

bitflags! {
    #[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    /// UNIX permissions
    pub struct Permissions: u16 {
        /// Other users have an executable permission
        const X_OTHER    = 0b00000000_00000001;
        /// Other users have a write permission
        const W_OTHER    = 0b00000000_00000010;
        /// Other users have a read permission
        const R_OTHER    = 0b00000000_00000100;
        /// Mask of the other permission bits
        const PERM_OTHER = 0b00000000_00000111;
        /// Users in a group have an executable permission
        const X_GROUP    = 0b00000000_00001000;
        /// Users in a group have a write permission
        const W_GROUP    = 0b00000000_00010000;
        /// Users in a group have a read permission
        const R_GROUP    = 0b00000000_00100000;
        /// Mask of the group permission bits
        const PERM_GROUP = 0b00000000_00111000;
        /// File owner has an executable permission
        const X_USER     = 0b00000000_01000000;
        /// File owner has a write permission
        const W_USER     = 0b00000000_10000000;
        /// File owner has a read permission
        const R_USER     = 0b00000001_00000000;
        /// Mask of the user permission bits
        const PERM_USER  = 0b00000001_11000000;
        /// Sticky bit - a restricted deletion flag
        const STICKY     = 0b00000010_00000000;
        /// Set-group-id bit
        const SET_GID    = 0b00000100_00000000;
        /// Set-user-id bit
        const SET_UID    = 0b00001000_00000000;
        /// Bits determining a type of the entry
        const TYPE_MASK  = 0b11110000_00000000;
        /// The entry is a directory
        const TYPE_DIR   = 0b01000000_00000000;
        /// The entry is a regular file
        const TYPE_FILE  = 0b10000000_00000000;
        /// The entry is a symbolic link
        const TYPE_LINK  = 0b10100000_00000000;
    }
}

impl Permissions {
    /// Return whether these are a directory entry permissions
    pub fn is_dir(self) -> bool {
        self.intersection(Permissions::TYPE_MASK) == Permissions::TYPE_DIR
    }
    /// Return whether these are a file entry permissions
    pub fn is_file(self) -> bool {
        self.intersection(Permissions::TYPE_MASK) == Permissions::TYPE_FILE
    }
    /// Return whether these are a symbolic link entry permissions
    pub fn is_link(self) -> bool {
        self.intersection(Permissions::TYPE_MASK) == Permissions::TYPE_LINK
    }
    /// Return only the file owner permission flags
    pub fn user(self) -> Self {
        self.intersection(Permissions::PERM_USER)
    }
    /// Return only the group permission flags
    pub fn group(self) -> Self {
        self.intersection(Permissions::PERM_GROUP)
    }
    /// Return only the other permission flags
    pub fn other(self) -> Self {
        self.intersection(Permissions::PERM_OTHER)
    }
    /// Return whether any executable bit is set.
    pub fn is_executable(self) -> bool {
        self.intersects(Permissions::X_OTHER|Permissions::X_GROUP|Permissions::X_USER)
    }
    /// Return whether any writable bit is set.
    pub fn is_writable(self) -> bool {
        self.intersects(Permissions::W_OTHER|Permissions::W_GROUP|Permissions::W_USER)
    }
    /// Return whether any readable bit is set.
    pub fn is_readable(self) -> bool {
        self.intersects(Permissions::R_OTHER|Permissions::R_GROUP|Permissions::R_USER)
    }
    /// Return whether a sticky flag is set.
    pub fn is_sticky(self) -> bool {
        self.intersects(Permissions::STICKY)
    }
    /// Return whether a set GID flag is set.
    pub fn is_set_gid(self) -> bool {
        self.intersects(Permissions::SET_GID)
    }
    /// Return whether a set UID flag is set.
    pub fn is_set_uid(self) -> bool {
        self.intersects(Permissions::SET_UID)
    }
}

impl fmt::Display for Permissions {
    #[inline(never)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags: [u8;10] = *b"-rwxrwxrwx";
        match self.intersection(Permissions::TYPE_MASK) {
            Permissions::TYPE_FILE => {}
            Permissions::TYPE_LINK => {
                flags[0] = b'l';
            }
            Permissions::TYPE_DIR => {
                flags[0] = b'd';
            }
            _ => {
                flags[0] = b'?';
            }
        }
        let mut mask = Permissions::PERM_USER;
        for i in (1..=7).step_by(3) {
            let perm = self.intersection(mask);
            if !perm.is_readable() {
                flags[i] = b'-';
            }
            if !perm.is_writable() {
                flags[i + 1] = b'-';
            }
            if !perm.is_executable() {
                flags[i + 2] = b'-';
            }
            mask = Permissions::from_bits_retain(mask.bits() >> 3);
        }
        if self.is_set_uid() {
            let f = &mut flags[3];
            *f = if *f == b'-' {
                b'S'
            }
            else {
                b's'
            };
        }
        if self.is_set_gid() {
            let f = &mut flags[6];
            *f = if *f == b'-' {
                b'S'
            }
            else {
                b's'
            };
        }
        if self.is_sticky() {
            let f = &mut flags[9];
            *f = if *f == b'-' {
                b'T'
            }
            else {
                b't'
            };            
        }
        let flags = str::from_utf8(&flags).unwrap();
        flags.fmt(f)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_permissions_works() {
        assert!(Permissions::X_OTHER.is_executable());
        assert!(Permissions::W_OTHER.is_writable());
        assert!(Permissions::R_OTHER.is_readable());
        assert!(Permissions::X_OTHER.other().is_executable());
        assert!(Permissions::W_OTHER.other().is_writable());
        assert!(Permissions::R_OTHER.other().is_readable());
        assert!(!Permissions::X_OTHER.group().is_executable());
        assert!(!Permissions::W_OTHER.group().is_writable());
        assert!(!Permissions::R_OTHER.group().is_readable());
        assert!(!Permissions::X_OTHER.user().is_executable());
        assert!(!Permissions::W_OTHER.user().is_writable());
        assert!(!Permissions::R_OTHER.user().is_readable());
        assert!(Permissions::PERM_OTHER.is_executable());
        assert!(Permissions::PERM_OTHER.is_writable());
        assert!(Permissions::PERM_OTHER.is_readable());
        assert!(Permissions::PERM_OTHER.other().is_executable());
        assert!(Permissions::PERM_OTHER.other().is_writable());
        assert!(Permissions::PERM_OTHER.other().is_readable());
        assert!(!Permissions::PERM_OTHER.group().is_executable());
        assert!(!Permissions::PERM_OTHER.group().is_writable());
        assert!(!Permissions::PERM_OTHER.group().is_readable());
        assert!(!Permissions::PERM_OTHER.user().is_executable());
        assert!(!Permissions::PERM_OTHER.user().is_writable());
        assert!(!Permissions::PERM_OTHER.user().is_readable());
        assert!(Permissions::X_GROUP.is_executable());
        assert!(Permissions::W_GROUP.is_writable());
        assert!(Permissions::R_GROUP.is_readable());
        assert!(Permissions::X_GROUP.group().is_executable());
        assert!(Permissions::W_GROUP.group().is_writable());
        assert!(Permissions::R_GROUP.group().is_readable());
        assert!(!Permissions::X_GROUP.other().is_executable());
        assert!(!Permissions::W_GROUP.other().is_writable());
        assert!(!Permissions::R_GROUP.other().is_readable());
        assert!(!Permissions::X_GROUP.user().is_executable());
        assert!(!Permissions::W_GROUP.user().is_writable());
        assert!(!Permissions::R_GROUP.user().is_readable());
        assert!(Permissions::PERM_GROUP.is_executable());
        assert!(Permissions::PERM_GROUP.is_writable());
        assert!(Permissions::PERM_GROUP.is_readable());
        assert!(Permissions::PERM_GROUP.group().is_executable());
        assert!(Permissions::PERM_GROUP.group().is_writable());
        assert!(Permissions::PERM_GROUP.group().is_readable());
        assert!(!Permissions::PERM_GROUP.other().is_executable());
        assert!(!Permissions::PERM_GROUP.other().is_writable());
        assert!(!Permissions::PERM_GROUP.other().is_readable());
        assert!(!Permissions::PERM_GROUP.user().is_executable());
        assert!(!Permissions::PERM_GROUP.user().is_writable());
        assert!(!Permissions::PERM_GROUP.user().is_readable());
        assert!(Permissions::X_USER.is_executable());
        assert!(Permissions::W_USER.is_writable());
        assert!(Permissions::R_USER.is_readable());
        assert!(Permissions::X_USER.user().is_executable());
        assert!(Permissions::W_USER.user().is_writable());
        assert!(Permissions::R_USER.user().is_readable());
        assert!(!Permissions::X_USER.group().is_executable());
        assert!(!Permissions::W_USER.group().is_writable());
        assert!(!Permissions::R_USER.group().is_readable());
        assert!(!Permissions::X_USER.other().is_executable());
        assert!(!Permissions::W_USER.other().is_writable());
        assert!(!Permissions::R_USER.other().is_readable());
        assert!(Permissions::PERM_USER.is_executable());
        assert!(Permissions::PERM_USER.is_writable());
        assert!(Permissions::PERM_USER.is_readable());
        assert!(Permissions::PERM_USER.user().is_executable());
        assert!(Permissions::PERM_USER.user().is_writable());
        assert!(Permissions::PERM_USER.user().is_readable());
        assert!(!Permissions::PERM_USER.group().is_executable());
        assert!(!Permissions::PERM_USER.group().is_writable());
        assert!(!Permissions::PERM_USER.group().is_readable());
        assert!(!Permissions::PERM_USER.other().is_executable());
        assert!(!Permissions::PERM_USER.other().is_writable());
        assert!(!Permissions::PERM_USER.other().is_readable());
        assert!(Permissions::STICKY.is_sticky());
        assert!(Permissions::SET_GID.is_set_gid());
        assert!(Permissions::SET_UID.is_set_uid());
        assert!(Permissions::TYPE_DIR.is_dir());
        assert!(!Permissions::TYPE_DIR.is_file());
        assert!(!Permissions::TYPE_DIR.is_link());
        assert!(Permissions::TYPE_FILE.is_file());
        assert!(!Permissions::TYPE_FILE.is_dir());
        assert!(!Permissions::TYPE_FILE.is_link());
        assert!(Permissions::TYPE_LINK.is_link());
        assert!(!Permissions::TYPE_LINK.is_dir());
        assert!(!Permissions::TYPE_LINK.is_file());
        assert!(!Permissions::TYPE_MASK.is_dir());
        assert!(!Permissions::TYPE_MASK.is_file());
        assert!(!Permissions::TYPE_MASK.is_link());
        assert!(!Permissions::empty().is_executable());
        assert!(!Permissions::empty().is_writable());
        assert!(!Permissions::empty().is_readable());
        assert!(!Permissions::empty().other().is_executable());
        assert!(!Permissions::empty().other().is_writable());
        assert!(!Permissions::empty().other().is_readable());
        assert!(!Permissions::empty().group().is_executable());
        assert!(!Permissions::empty().group().is_writable());
        assert!(!Permissions::empty().group().is_readable());
        assert!(!Permissions::empty().user().is_executable());
        assert!(!Permissions::empty().user().is_writable());
        assert!(!Permissions::empty().user().is_readable());
        assert!(!Permissions::empty().is_sticky());
        assert!(!Permissions::empty().is_set_gid());
        assert!(!Permissions::empty().is_set_uid());
        assert!(!Permissions::empty().is_dir());
        assert!(!Permissions::empty().is_file());
        assert!(!Permissions::empty().is_dir());
        assert!(!Permissions::empty().is_file());
        assert!(!Permissions::empty().is_link());
        assert_eq!(format!("{}", Permissions::empty()),    "?---------");
        assert_eq!(format!("{}", Permissions::X_OTHER),    "?--------x");
        assert_eq!(format!("{}", Permissions::W_OTHER),    "?-------w-");
        assert_eq!(format!("{}", Permissions::R_OTHER),    "?------r--");
        assert_eq!(format!("{}", Permissions::PERM_OTHER), "?------rwx");
        assert_eq!(format!("{}", Permissions::X_GROUP),    "?-----x---");
        assert_eq!(format!("{}", Permissions::W_GROUP),    "?----w----");
        assert_eq!(format!("{}", Permissions::R_GROUP),    "?---r-----");
        assert_eq!(format!("{}", Permissions::PERM_GROUP), "?---rwx---");
        assert_eq!(format!("{}", Permissions::X_USER),     "?--x------");
        assert_eq!(format!("{}", Permissions::W_USER),     "?-w-------");
        assert_eq!(format!("{}", Permissions::R_USER),     "?r--------");
        assert_eq!(format!("{}", Permissions::PERM_USER),  "?rwx------");
        assert_eq!(format!("{}", Permissions::STICKY),     "?--------T");
        assert_eq!(format!("{}", Permissions::SET_GID),    "?-----S---");
        assert_eq!(format!("{}", Permissions::SET_UID),    "?--S------");
        assert_eq!(format!("{}", Permissions::TYPE_MASK),  "?---------");
        assert_eq!(format!("{}", Permissions::TYPE_DIR),   "d---------");
        assert_eq!(format!("{}", Permissions::TYPE_FILE),  "----------");
        assert_eq!(format!("{}", Permissions::TYPE_LINK),  "l---------");
        assert_eq!(format!("{}", Permissions::TYPE_LINK|
                                 Permissions::PERM_OTHER|
                                 Permissions::PERM_GROUP|
                                 Permissions::PERM_USER),  "lrwxrwxrwx");
        assert_eq!(format!("{}", Permissions::TYPE_DIR|
                                 Permissions::PERM_OTHER|
                                 Permissions::PERM_GROUP|
                                 Permissions::PERM_USER|
                                 Permissions::STICKY|
                                 Permissions::SET_GID|
                                 Permissions::SET_UID),    "drwsrwsrwt");
        assert_eq!(format!("{}", Permissions::TYPE_DIR|
                                 Permissions::X_OTHER|
                                 Permissions::X_GROUP|
                                 Permissions::X_USER|
                                 Permissions::STICKY|
                                 Permissions::SET_GID|
                                 Permissions::SET_UID),    "d--s--s--t");
        assert_eq!(format!("{}", Permissions::TYPE_FILE|
                                 Permissions::R_OTHER|
                                 Permissions::R_GROUP|
                                 Permissions::R_USER|
                                 Permissions::STICKY|
                                 Permissions::SET_GID|
                                 Permissions::SET_UID),    "-r-Sr-Sr-T");
        assert_eq!(format!("{}", Permissions::TYPE_FILE|
                                 Permissions::W_OTHER|
                                 Permissions::W_GROUP|
                                 Permissions::W_USER|
                                 Permissions::STICKY|
                                 Permissions::SET_GID|
                                 Permissions::SET_UID),    "--wS-wS-wT");
        assert_eq!(format!("{}", Permissions::all()),      "?rwsrwsrwt");
    }
}
