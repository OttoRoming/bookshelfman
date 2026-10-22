package storygraph

import (
	"github.com/google/uuid"
)

type Cache struct {
	Version uint8
	Author  map[uuid.UUID]Author
}

const (
	cacheVersion uint8 = 1
)

func NewCache() *Cache {
	return &Cache{
		Version: cacheVersion,
		Author:  make(map[uuid.UUID]Author),
	}
}

func (c *Cache) GetAuthor(id uuid.UUID) (Author, bool) {
	author, found := c.Author[id]
	return author, found
}

func (c *Cache) SetAuthor(author Author) {
	c.Author[author.ID] = author
}
