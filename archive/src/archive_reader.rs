use std::io::{Cursor, Read, Seek};

use mla::ArchiveReader;

use crate::{
    constants::BINCODE_CONFIG,
    errors::{NextTileChunkError, PlacedArchiveError},
    structures::{DecodedTilePlacement, Meta, StoredTilePlacement},
};

pub struct PlacedArchiveReader<'a, R: Read + Seek> {
    mla: ArchiveReader<'a, R>,
    pub meta: Meta,
    current_tile_chunk_id: Option<u32>,
    current_tile_chunk_data: Option<Cursor<Vec<u8>>>,
}

impl<'a, R: Read + Seek + 'a> PlacedArchiveReader<'a, R> {
    pub fn new(reader: R) -> Result<Self, PlacedArchiveError> {
        let mut mla = match ArchiveReader::new(reader) {
            Ok(mla) => mla,
            Err(err) => return Err(PlacedArchiveError::MLAReadError(err)),
        };

        let mut meta_file = match mla.get_file("meta".to_string()) {
            Ok(Some(meta_file)) => meta_file,
            Ok(None) => return Err(PlacedArchiveError::MissingMetaFile),
            Err(_) => return Err(PlacedArchiveError::MissingMetaFile),
        };

        let meta: Meta = match bincode::decode_from_std_read(&mut meta_file.data, BINCODE_CONFIG) {
            Ok(meta) => meta,
            Err(_) => return Err(PlacedArchiveError::CouldNotDecodeMetaFile),
        };

        Ok(Self {
            mla,
            meta,
            current_tile_chunk_id: None,
            current_tile_chunk_data: None,
        })
    }

    fn get_next_chunk_data(&mut self) -> Result<(), NextTileChunkError> {
        let tile_chunk_id = match self.current_tile_chunk_id {
            Some(id) => id + 1,
            None => 0,
        };

        if tile_chunk_id >= self.meta.chunk_descs.len() as u32 {
            return Err(NextTileChunkError::OutOfChunks);
        }

        let tile_chunk_file_name = format!("tiles/{}", tile_chunk_id);

        let mut current_tile_chunk_file = match self.mla.get_file(tile_chunk_file_name) {
            Ok(Some(tile_chunk_file)) => tile_chunk_file,
            Ok(None) => return Err(NextTileChunkError::MissingChunkFile),
            Err(err) => return Err(NextTileChunkError::CouldNotFetchChunkFile(err)),
        };

        self.current_tile_chunk_id = Some(tile_chunk_id);

        let mut buf = Vec::new();
        current_tile_chunk_file.data.read_to_end(&mut buf).unwrap();

        self.current_tile_chunk_data = Some(Cursor::new(buf));

        Ok(())
    }
}

impl<'a, R: Read + Seek> Iterator for PlacedArchiveReader<'a, R> {
    type Item = DecodedTilePlacement;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.current_tile_chunk_data {
            Some(ref mut data) => {
                if data.position() == data.get_ref().len() as u64 {
                    match self.get_next_chunk_data() {
                        Ok(_) => self.next(),
                        Err(_) => None,
                    }
                } else {
                    let tile_placement: StoredTilePlacement =
                        match bincode::decode_from_std_read(data, BINCODE_CONFIG) {
                            Ok(tile_placement) => tile_placement,
                            Err(_) => return None,
                        };

                    Some(DecodedTilePlacement {
                        x: tile_placement.x,
                        y: tile_placement.y,
                        ms_since_epoch: tile_placement.ms_since_epoch,
                        color: *self
                            .meta
                            .color_id_to_tuple
                            .get(&tile_placement.color_index)
                            .unwrap(),
                    })
                }
            }
            None => match self.get_next_chunk_data() {
                Ok(_) => self.next(),
                Err(_) => None,
            },
        }
    }
}
