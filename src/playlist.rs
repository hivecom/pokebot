use std::collections::VecDeque;

use tracing::{Span, info};

use crate::youtube_dl::AudioMetadata;

pub struct Playlist {
    data: VecDeque<AudioMetadata>,
    span: Span,
}

impl Playlist {
    pub fn new(span: Span) -> Self {
        Self {
            data: VecDeque::new(),
            span,
        }
    }

    pub fn push(&mut self, data: AudioMetadata) {
        info!(
            parent: &self.span,
            title = &data.title,
            artist = &data.artist,
            "Adding to playlist"
        );

        self.data.push_front(data)
    }

    pub fn pop(&mut self) -> Option<AudioMetadata> {
        let res = self.data.pop_back();
        info!(
            parent: &self.span,
            title = res.as_ref().map(|r| &r.title),
            "Popping from playlist",
        );

        res
    }

    pub fn get(&mut self, idx: usize) -> Option<&AudioMetadata> {
        let res = self.data.get(idx);
        info!(
            parent: &self.span,
            title = res.as_ref().map(|r| &r.title),
            "Getting from playlist",
        );

        res
    }

    pub fn to_vec(&self) -> Vec<AudioMetadata> {
        let (a, b) = self.data.as_slices();

        let mut res = a.to_vec();
        res.extend_from_slice(b);
        res.reverse();

        res
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();

        info!(parent: &self.span, "Cleared playlist");
    }

    pub fn top_id(&self) -> i64 {
        self.data.iter().map(|a| a.id).max().unwrap_or(0)
    }
}
