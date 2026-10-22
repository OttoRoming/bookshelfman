package storygraph

import (
	"fmt"
	"github.com/PuerkitoBio/goquery"
	"github.com/google/uuid"
	"io"
	"net/url"
)

type BookPane struct {
	ID       uuid.UUID
	Authors  []uuid.UUID
	Title    string
	CoverURL *url.URL
}

func splitUUIDs(input string) ([]uuid.UUID, error) {
	if len(input) == 0 {
		return nil, nil
	}

	var uuids []uuid.UUID
	i := 0
	for i < len(input) {
		if i+36 > len(input) {
			break
		}

		parsedUUID, err := uuid.Parse(input[i : i+36])
		if err != nil {
			return nil, fmt.Errorf("failed to parse UUID at index %d: %w", i, err)
		}

		uuids = append(uuids, parsedUUID)
		i += 37
	}

	return uuids, nil
}

func (s *Storygraph) scrapeSearch(body io.ReadCloser) ([]BookPane, error) {
	var books []BookPane

	doc, err := goquery.NewDocumentFromReader(body)
	if err != nil {
		return books, err
	}

	doc.Find("div[data-book-id][data-author-ids]").Each(func(i int, s *goquery.Selection) {
		var book BookPane
		bookIDStr, _ := s.Attr("data-book-id")
		bookID, err := uuid.Parse(bookIDStr)
		if err != nil {
			return
		}
		book.ID = bookID

		s.Find(".book-title-author-and-series h3 a").Each(func(i int, s *goquery.Selection) {
			book.Title = s.Text()
		})

		s.Find(".book-cover a img").Each(func(i int, s *goquery.Selection) {
			if src, exists := s.Attr("src"); exists {
				u, err := url.Parse(src)
				if err != nil {
					return
				}

				book.CoverURL = u
			}
		})

		authorIDsStr, _ := s.Attr("data-author-ids")
		book.Authors, err = splitUUIDs(authorIDsStr)
		if err != nil {
			return
		}

		books = append(books, book)
	})

	return books, nil
}

func (s *Storygraph) Search(query string) ([]BookPane, error) {
	params := url.Values{}
	params.Add("search_term", query)

	body, err := s.fetch("/browse", &params)
	if err != nil {
		return []BookPane{}, err
	}

	return s.scrapeSearch(body)
}
