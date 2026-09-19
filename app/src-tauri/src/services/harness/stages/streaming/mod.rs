// NOTE: Tag demuxing (StreamingTagDemuxer) is intentionally deferred.
// Text flows directly through ClauseChunker to TTS for normal robust pipeline testing.

pub mod chunker;
pub mod normalizer;
pub mod router;

pub use chunker::ClauseChunker;
pub use normalizer::TextNormalizer;
pub use router::{StreamRoutingHandles, StreamRoutingStage};
