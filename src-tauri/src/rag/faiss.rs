use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

const MAGIC_HEADER: &[u8; 8] = b"SKVEC_01";

pub struct VectorIndex {
    dimension: u32,
    vectors: Vec<Vec<f32>>,
}

impl VectorIndex {
    pub fn new(dimension: u32) -> Self {
        Self {
            dimension,
            vectors: Vec::new(),
        }
    }

    pub fn add(&mut self, vector: &[f32]) {
        if self.vectors.is_empty() {
            self.dimension = vector.len() as u32;
        }
        if vector.len() == self.dimension as usize {
            self.vectors.push(vector.to_vec());
        } else {
            eprintln!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            );
        }
    }

    pub fn search_with_candidates(
        &self,
        query: &[f32],
        candidate_ids: &[u64],
        k: usize,
    ) -> Vec<u64> {
        if self.vectors.is_empty() {
            eprintln!("VectorIndex search: index has no vectors");
            return Vec::new();
        }
        if query.len() != self.dimension as usize {
            eprintln!(
                "VectorIndex search: dimension mismatch (query dimension: {}, index dimension: {})",
                query.len(),
                self.dimension
            );
            return Vec::new();
        }

        let mut scored: Vec<(u64, f32)> = candidate_ids
            .iter()
            .filter_map(|&id| {
                let vec = self.vectors.get(id as usize)?;
                if vec.len() != self.dimension as usize {
                    return None;
                }
                let dist: f32 = query
                    .iter()
                    .zip(vec.iter())
                    .map(|(a, b)| {
                        let diff = a - b;
                        diff * diff
                    })
                    .sum();
                Some((id, dist))
            })
            .collect();

        // Sort ascending by distance (nearest / highest similarity first)
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(k).map(|(id, _)| id).collect()
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<u64> {
        let all_ids: Vec<u64> = (0..self.vectors.len() as u64).collect();
        self.search_with_candidates(query, &all_ids, k)
    }

    pub fn save(&self, path: &str) {
        if let Ok(file) = File::create(path) {
            let mut writer = BufWriter::new(file);
            let _ = writer.write_all(MAGIC_HEADER);
            let _ = writer.write_all(&self.dimension.to_le_bytes());
            let count = self.vectors.len() as u64;
            let _ = writer.write_all(&count.to_le_bytes());

            for vec in &self.vectors {
                for &val in vec {
                    let _ = writer.write_all(&val.to_le_bytes());
                }
            }
            let _ = writer.flush();
        }
    }

    pub fn load(path: &str) -> Self {
        if let Ok(file) = File::open(path) {
            let mut reader = BufReader::new(file);
            let mut magic = [0u8; 8];
            if reader.read_exact(&mut magic).is_ok() && &magic == MAGIC_HEADER {
                let mut dim_bytes = [0u8; 4];
                let mut count_bytes = [0u8; 8];

                if reader.read_exact(&mut dim_bytes).is_ok()
                    && reader.read_exact(&mut count_bytes).is_ok()
                {
                    let dimension = u32::from_le_bytes(dim_bytes);
                    let count = u64::from_le_bytes(count_bytes);

                    let mut vectors = Vec::with_capacity(count as usize);
                    let mut valid = true;

                    for _ in 0..count {
                        let mut vec = Vec::with_capacity(dimension as usize);
                        for _ in 0..dimension {
                            let mut val_bytes = [0u8; 4];
                            if reader.read_exact(&mut val_bytes).is_ok() {
                                vec.push(f32::from_le_bytes(val_bytes));
                            } else {
                                valid = false;
                                break;
                            }
                        }
                        if !valid {
                            break;
                        }
                        vectors.push(vec);
                    }

                    if valid {
                        return Self { dimension, vectors };
                    }
                }
            }
        }

        // Graceful fallback if file does not exist or was created with legacy C++ Faiss format
        Self::new(768)
    }

    pub fn ntotal(&self) -> u64 {
        self.vectors.len() as u64
    }

    pub fn get_vector(&self, id: usize) -> Option<&Vec<f32>> {
        self.vectors.get(id)
    }

    pub fn set_vectors(&mut self, vectors: Vec<Vec<f32>>) {
        self.vectors = vectors;
    }

    pub fn clear(&mut self) {
        self.vectors.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_index_add_search_save_load() {
        let mut index = VectorIndex::new(3);
        index.add(&[1.0, 0.0, 0.0]); // id 0
        index.add(&[0.0, 1.0, 0.0]); // id 1
        index.add(&[0.0, 0.0, 1.0]); // id 2

        assert_eq!(index.ntotal(), 3);

        // Searching for something closest to [0.9, 0.1, 0.0]
        let results = index.search(&[0.9, 0.1, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], 0); // nearest should be id 0

        // Test save and load
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_index.vec");
        let path_str = path.to_str().unwrap();

        index.save(path_str);

        let loaded = VectorIndex::load(path_str);
        assert_eq!(loaded.ntotal(), 3);
        assert_eq!(loaded.dimension, 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_vector_index_search_with_candidates() {
        let mut index = VectorIndex::new(3);
        index.add(&[1.0, 0.0, 0.0]); // id 0 (Book A)
        index.add(&[0.9, 0.1, 0.0]); // id 1 (Book A)
        index.add(&[0.0, 1.0, 0.0]); // id 2 (Book B)
        index.add(&[0.0, 0.9, 0.1]); // id 3 (Book B)

        // Query close to id 0 and 1
        let query = [0.95, 0.05, 0.0];

        // Global search returns id 0 and 1
        let global_results = index.search(&query, 2);
        assert_eq!(global_results, vec![0, 1]);

        // Filtered search restricted to Book B (candidates 2 and 3)
        let filtered_results = index.search_with_candidates(&query, &[2, 3], 2);
        assert_eq!(filtered_results, vec![3, 2]);
    }
}



