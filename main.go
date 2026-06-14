package main

import (
	// "fmt"

	"github.com/OttoRoming/bookshelfman/manager"
	"github.com/alecthomas/kong"
	// "github.com/OttoRoming/bookshelfman/storygraph"
)

var CLI struct {
	Init struct {
		Path string `arg:"" name:"path" help:"Path to bookshelf." type:"path"`
	} `cmd:"" help:"Remove files."`
}

func main() {
	ctx := kong.Parse(&CLI)

	switch ctx.Command() {
	case "init <path>":
		manager.Init(CLI.Init.Path)
	default:
		ctx.PrintUsage(false)
	}

	// _, err := manager.New()
	// if err != nil {
	// 	panic(err)
	// }

	// s, err := storygraph.New()
	// if err != nil {
	// 	panic(err)
	// }
	//
	// res, err := s.Search("the great gatsby")
	// if err != nil {
	// 	panic(err)
	// }
	//
	// for _, book := range res {
	// 	fmt.Printf("book: %v\n", book)
	// }
}
