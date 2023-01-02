use bincode::config::{Configuration, Fixint, LittleEndian, NoLimit, WriteFixedArrayLength};

// Use legacy encoding for fixed-width integers (field size needs to be constant so we can seek)
pub const BINCODE_CONFIG: Configuration<LittleEndian, Fixint, WriteFixedArrayLength, NoLimit> =
    bincode::config::legacy();
