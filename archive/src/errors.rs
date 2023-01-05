#[derive(Debug)]
pub enum PlacedArchiveError {
    MLAReadError(mla::errors::Error),
    MissingMetaFile,
    CouldNotDecodeMetaFile,
}

#[derive(Debug)]
pub enum NextTileChunkError {
    OutOfChunks,
    MissingChunkFile,
    CouldNotFetchChunkFile(mla::errors::Error),
}
