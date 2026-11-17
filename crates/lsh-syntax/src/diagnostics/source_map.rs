pub struct Source {
    pub name: String,
    pub contents: String,
}

#[derive(Default)]
pub struct SourceMap {
    sources: Vec<Option<Source>>,
    free_ids: Vec<usize>,
}

impl SourceMap {
    pub fn insert(&mut self, source_name: &str, source: &str) -> usize {
        let src = Source {
            name: source_name.to_owned(),
            contents: source.to_owned(),
        };

        if let Some(id) = self.free_ids.pop() {
            self.sources[id] = Some(src);
            id
        } else {
            self.sources.push(Some(src));
            self.sources.len() - 1
        }
    }

    pub fn remove(&mut self, id: usize) -> Option<Source> {
        let src = self.sources.get_mut(id)?.take()?;
        self.free_ids.push(id);
        Some(src)
    }

    pub fn get(&self, id: usize) -> Option<&Source> {
        self.sources.get(id).and_then(Option::as_ref)
    }
}
