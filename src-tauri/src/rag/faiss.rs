use faiss::{index_factory, MetricType, Index};
use faiss::index::IndexImpl;

pub struct VectorIndex {
    pub index: IndexImpl,
}

impl VectorIndex {
    pub fn new(dimension: u32) -> Self {
        let index = index_factory(dimension, "HNSW32", MetricType::L2).unwrap();
        Self { index }
    }

    pub fn add(&mut self, vectors: &[f32]) {
        self.index.add(vectors).unwrap();
    }

    pub fn search(&mut self, query: &[f32], k: usize) -> Vec<u64> {
        let result = self.index.search(query, k).unwrap();
        result.labels.iter().map(|&l| l.get().unwrap_or(u64::MAX)).collect()
    }

    pub fn save(&self, path: &str) {
        faiss::write_index(&self.index, path).unwrap();
    }

    pub fn load(path: &str) -> Self {
        let index = faiss::read_index(path).unwrap();
        Self { index }
    }

    pub fn ntotal(&self) -> u64 {
        self.index.ntotal() as u64
    }

}

