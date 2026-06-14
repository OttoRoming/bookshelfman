package storygraph

import (
	"fmt"
	http "github.com/bogdanfinn/fhttp"
	tls_client "github.com/bogdanfinn/tls-client"
	"github.com/bogdanfinn/tls-client/profiles"
)

type Storygraph struct {
	client tls_client.HttpClient
	header http.Header
}

func New() (*Storygraph, error) {
	jar := tls_client.NewCookieJar()
	options := []tls_client.HttpClientOption{
		tls_client.WithTimeoutSeconds(30),
		tls_client.WithClientProfile(profiles.Firefox_148),
		tls_client.WithNotFollowRedirects(),
		tls_client.WithCookieJar(jar),
	}

	client, err := tls_client.NewHttpClient(tls_client.NewNoopLogger(), options...)
	if err != nil {
		return nil, err
	}

	req, err := http.NewRequest(http.MethodGet, "https://thestorygraph.com/", nil)
	if err != nil {
		return nil, err
	}

	header := http.Header{
		"accept":                    {"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"},
		"accept-encoding":           {"gzip, deflate, br, zstd"},
		"accept-language":           {"en-US,en;q=0.9"},
		"connection":                {"keep-alive"},
		"priority":                  {"u=0, i"},
		"sec-fetch-dest":            {"document"},
		"sec-fetch-mode":            {"navigate"},
		"sec-fetch-site":            {"none"},
		"sec-fetch-user":            {"?1"},
		"sec-gpc":                   {"1"},
		"upgrade-insecure-requests": {"1"},
		"user-agent":                {"Mozilla/5.0 (X11; Linux x86_64; rv:151.0) Gecko/20100101 Firefox/148.0"},
		http.HeaderOrderKey: {
			"accept",
			"accept-encoding",
			"accept-language",
			"connection",
			"priority",
			"sec-fetch-dest",
			"sec-fetch-mode",
			"sec-fetch-site",
			"sec-fetch-user",
			"sec-gpc",
			"upgrade-insecure-requests",
			"user-agent",
		},
	}
	req.Header = header

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}

	// check that we got the cookie we need to access the site
	found := false
	for _, cookie := range jar.Cookies(resp.Request.URL) {
		fmt.Printf("cookie: %v\n", cookie)
		if cookie.Name == "_storygraph_marketing_session" {
			found = true
			break
		}
	}
	if !found {
		return nil, fmt.Errorf("required cookie not found")
	}

	return &Storygraph{
		client: client,
		header: header,
	}, nil
}
