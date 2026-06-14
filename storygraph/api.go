package storygraph

import (
	"fmt"
	"strings"

	"github.com/google/uuid"
)

type Author struct {
	ID   uuid.UUID
	Name string
}

func (a Author) String() string {
	return a.ID.String()
}

type Book struct {
	ID       uuid.UUID
	Title    string
	Authors  []Author
	CoverURL string
}

func (b Book) String() string {
	authors := make([]string, len(b.Authors))
	for i, author := range b.Authors {
		authors[i] = author.String()
	}
	return fmt.Sprintf("%s - %s", b.ID, strings.Join(authors, ", "))
}
