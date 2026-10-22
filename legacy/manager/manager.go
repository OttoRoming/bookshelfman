package manager

import (
	"charm.land/huh/v2"
	"github.com/adrg/xdg"
	"github.com/pelletier/go-toml/v2"
	"log/slog"
	"os"
)

var (
	configDir  = xdg.ConfigHome + "/bookshelfman"
	configPath = configDir + "/bookshelfman.toml"
)

type Config struct {
	BookshelfPath string `toml:"bookshelf_path"`
}

type Author struct {
	Name string
}

type Book struct {
	Title string
	// Authors
}

type Manager struct {
	config Config
	// books  []Book
	slog *slog.Logger
}

func New() (*Manager, error) {
	s := slog.New(slog.NewTextHandler(os.Stderr, nil))

	f, err := os.Open(configPath)
	if err != nil {
		s.Error("Failed to read config file, to intitialize bookshelfman run `bookshelfman init <path>`", "path", configPath, "error", err)
	}

	var config Config
	toml.NewDecoder(f).Decode(&config)

	return &Manager{
		config: config,
		slog:   s,
	}, nil
}

func Init(path string) {
	s := slog.New(slog.NewTextHandler(os.Stderr, nil))

	var confirmed bool
	form := huh.NewConfirm().
		Title("Initialize Bookshelf Manager").
		Description("This will create a config file at `" + configPath + "`\nand set the bookshelf path to `" + path + "`.\nDo you want to continue?").
		Value(&confirmed)

	err := form.Run()
	if err != nil {
		s.Error("Failed to run form", "error", err)
		return
	}

	if !confirmed {
		s.Info("Initialization cancelled")
		return
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		err := os.MkdirAll(path, os.ModePerm)
		if err != nil {
			s.Error("Failed to create directory", "path", path, "error", err)
			return
		}
	}

	if _, err := os.Stat(configDir); os.IsNotExist(err) {
		err := os.MkdirAll(configDir, os.ModePerm)
		if err != nil {
			s.Error("Failed to create config directory", "path", configDir, "error", err)
			return
		}
	}

	f, err := os.Create(configPath)
	if err != nil {
		s.Error("Failed to create config file", "path", configPath, "error", err)
		return
	}
	defer f.Close()

	config := Config{
		BookshelfPath: path,
	}

	toml.NewEncoder(f).Encode(config)
}
