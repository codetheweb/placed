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

impl<'a, R: Read + Seek> Read for PlacedArchiveReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.current_tile_chunk_data {
            Some(ref mut data) => {
                if data.position() == data.get_ref().len() as u64 {
                    match self.get_next_chunk_data() {
                        Ok(_) => self.read(buf),
                        Err(_) => Ok(0),
                    }
                } else {
                    data.read(buf)
                }
            }
            None => match self.get_next_chunk_data() {
                Ok(_) => self.read(buf),
                Err(_) => Ok(0),
            },
        }
    }
}

impl<'a, R: Read + Seek> Iterator for PlacedArchiveReader<'a, R> {
    type Item = DecodedTilePlacement;

    fn next(&mut self) -> Option<Self::Item> {
        let tile_placement: StoredTilePlacement =
            match bincode::decode_from_std_read(self, BINCODE_CONFIG) {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::NaiveDateTime;
    use rand::Rng;
    use tempfile::NamedTempFile;

    use crate::{structures::StoredTilePlacement, PlacedArchiveReader};

    #[test]
    fn read_trait() {
        let writeable_file = NamedTempFile::new().unwrap();
        let readable_file = writeable_file.reopen().unwrap();
        let mut archive_writer = crate::PlacedArchiveWriter::new(writeable_file);

        let canvas_size = 512;
        let required_num_of_tile_updates = (canvas_size as u32) * (canvas_size as u32);

        let mut color_id_to_tuple = BTreeMap::new();
        color_id_to_tuple.insert(0, [0, 0, 0, 255]);
        color_id_to_tuple.insert(1, [255, 0, 0, 255]);
        color_id_to_tuple.insert(2, [0, 255, 0, 255]);
        color_id_to_tuple.insert(3, [0, 0, 255, 255]);
        color_id_to_tuple.insert(4, [255, 255, 0, 255]);
        color_id_to_tuple.insert(5, [255, 0, 255, 255]);
        color_id_to_tuple.insert(6, [0, 255, 255, 255]);
        color_id_to_tuple.insert(7, [255, 255, 255, 255]);

        let mut generator = rand::thread_rng();
        let mut expected_tiles: Vec<StoredTilePlacement> = Vec::new();

        for i in 0..required_num_of_tile_updates {
            let tile = StoredTilePlacement {
                x: generator.gen_range(0..canvas_size),
                y: generator.gen_range(0..canvas_size),
                color_index: generator.gen_range(0..color_id_to_tuple.len() as u8),
                ms_since_epoch: i,
            };

            archive_writer.add_tile(
                tile.x,
                tile.y,
                *color_id_to_tuple.get(&tile.color_index).unwrap(),
                NaiveDateTime::from_timestamp_millis(tile.ms_since_epoch as i64).unwrap(),
            );
            expected_tiles.push(tile);
        }

        archive_writer.finalize(false);

        let reader = PlacedArchiveReader::new(readable_file).unwrap();
        let read_tiles = reader.collect::<Vec<_>>();
        for (i, expected_tile) in expected_tiles.into_iter().enumerate() {
            let read_tile = &read_tiles[i];

            assert_eq!(read_tile.x, expected_tile.x);
            assert_eq!(read_tile.y, expected_tile.y);
            assert_eq!(
                read_tile.color,
                *color_id_to_tuple.get(&expected_tile.color_index).unwrap()
            );
            assert_eq!(read_tile.ms_since_epoch, expected_tile.ms_since_epoch);
        }
    }
}
