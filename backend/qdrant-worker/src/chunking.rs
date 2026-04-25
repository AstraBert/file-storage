use bm25::Embedding;
use memchunk::chunk;

#[derive(Debug)]
pub struct Chunk {
    pub content: String,
    pub embedding: Option<Embedding>,
}

impl Chunk {
    pub fn from_content(content: String) -> Self {
        Self {
            content,
            embedding: None,
        }
    }
}

pub fn chunk_text(text: &str, size: usize) -> Vec<Chunk> {
    let text_bytes = text.as_bytes();
    let chunks: Vec<&[u8]> = chunk(text_bytes).size(size).collect();
    let string_chunks: Vec<String> = chunks
        .iter()
        .map(|&chunk| String::from_utf8_lossy(chunk).to_string())
        .collect();
    let mut struct_chunks: Vec<Chunk> = vec![];
    for c in string_chunks {
        let chunk_struct = Chunk::from_content(c);
        struct_chunks.push(chunk_struct);
    }
    println!("Created {:?} chunks", struct_chunks.len());
    struct_chunks
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_chunk_from_content() {
        let chunk = Chunk::from_content("test".to_string());
        assert_eq!(chunk.content, "test".to_string());
        assert!(chunk.embedding.is_none());
    }

    #[test]
    fn test_chunk_text() {
        // this config should produce only one chunk
        let text = "This is a one-chunk text.";
        let size: usize = 1024;
        let chunks = chunk_text(text, size);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "This is a one-chunk text.".to_string());
    }
}
