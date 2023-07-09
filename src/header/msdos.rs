use bitflags::bitflags;

bitflags! {
    #[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
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
