use std::collections::HashMap;

use uuid::Uuid;

use super::Author;

/// In-memory cache of StoryGraph metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    pub version: u8,
    pub author: HashMap<Uuid, Author>,
}

impl Cache {
    pub const VERSION: u8 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            author: HashMap::new(),
        }
    }

    pub fn get_author(&self, id: Uuid) -> Option<&Author> {
        self.author.get(&id)
    }

    pub fn set_author(&mut self, author: Author) {
        self.author.insert(author.id, author);
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn author(id: Uuid) -> Author {
        Author {
            id,
            name: "Ursula K. Le Guin".to_owned(),
        }
    }

    #[test]
    fn stores_and_retrieves_authors() {
        let id = Uuid::new_v4();
        let mut cache = Cache::new();

        assert_eq!(cache.version, Cache::VERSION);
        assert_eq!(cache.get_author(id), None);

        cache.set_author(author(id));

        assert_eq!(cache.get_author(id), Some(&author(id)));
    }
}
