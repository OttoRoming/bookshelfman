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

	Add struct {
		Paths []string `arg:"" name:"paths" help:"Path to book." type:"paths"`
	} `cmd:"" help:"Add a book to the bookshelf."`
}

func main() {
	ctx := kong.Parse(&CLI)

	switch ctx.Command() {
	case "init <path>":
		manager.Init(CLI.Init.Path)
	case "add <paths>":
		m, err := manager.New()
		if err != nil {
			panic(err)
		}

		err = m.Add(CLI.Add.Paths)
		if err != nil {
			panic(err)
		}
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
