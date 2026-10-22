type OpenLibrarySearchResponse struct {
	NumFound         int    `json:"numFound"`
	Start            int    `json:"start"`
	NumFoundExact    bool   `json:"numFoundExact"`
	NumFoundSnake    int    `json:"num_found"`
	DocumentationURL string `json:"documentation_url"`
	Q                string `json:"q"`
	Offset           *int   `json:"offset"`
	Docs             []Doc  `json:"docs"`
}

type Doc struct {
	AuthorKey          []string `json:"author_key,omitempty"`
	AuthorName         []string `json:"author_name,omitempty"`
	CoverEditionKey    string   `json:"cover_edition_key,omitempty"`
	CoverI             int      `json:"cover_i,omitempty"`
	EbookAccess        string   `json:"ebook_access,omitempty"`
	EditionCount       int      `json:"edition_count,omitempty"`
	FirstPublishYear   int      `json:"first_publish_year,omitempty"`
	HasFulltext        bool     `json:"has_fulltext,omitempty"`
	IA                 []string `json:"ia,omitempty"`
	IACollection       []string `json:"ia_collection,omitempty"`
	Key                string   `json:"key,omitempty"`
	Language           []string `json:"language,omitempty"`
	LendingEditionS    string   `json:"lending_edition_s,omitempty"`
	LendingIdentifierS string   `json:"lending_identifier_s,omitempty"`
	PublicScanB        bool     `json:"public_scan_b,omitempty"`
	Title              string   `json:"title,omitempty"`
	Subtitle           string   `json:"subtitle,omitempty"`
}
