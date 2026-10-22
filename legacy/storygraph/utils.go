package storygraph

import (
	"fmt"
	"io"
	"net/url"
	"strings"

	http "github.com/bogdanfinn/fhttp"
)

func (s *Storygraph) fetch(path string, params *url.Values) (io.ReadCloser, error) {
	if params == nil {
		params = &url.Values{}
	}

	u := url.URL{
		Scheme:   "https",
		Host:     "app.thestorygraph.com",
		Path:     path,
		RawQuery: params.Encode(),
	}

	req, err := http.NewRequest("GET", u.String(), nil)
	if err != nil {
		return nil, err
	}
	req.Header = s.header

	resp, err := s.client.Do(req)
	if err != nil {
		return nil, err
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("unexpected status code: %d", resp.StatusCode)
	}

	contentType := resp.Header.Get("Content-Type")
	if !strings.Contains(contentType, "text/html") {
		return nil, fmt.Errorf("unexpected content type: %s", contentType)
	}

	return resp.Body, nil
}
