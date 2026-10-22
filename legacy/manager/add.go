package manager

import (
	_ "fmt"
	"os"

	"charm.land/bubbles/v2/textinput"
	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/OttoRoming/bookshelfman/storygraph"
)

type BookAddModel struct {
	storygraph *storygraph.Storygraph
	textInput  textinput.Model
	query      string
	bookPanes  []storygraph.BookPane
	err        error
}

func initialBookAddModel() (BookAddModel, error) {
	ti := textinput.New()
	ti.Focus()

	s, err := storygraph.New()
	if err != nil {
		return BookAddModel{}, err
	}

	model := BookAddModel{
		storygraph: s,
		textInput:  ti,
		query:      "",
	}

	return model, nil
}

func (m BookAddModel) Init() tea.Cmd {
	return nil
}

func (m BookAddModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd

	switch msg := msg.(type) {
	case tea.KeyPressMsg:
		switch msg.String() {
		case "ctrl+c", "esc":
			return m, tea.Quit
		}
	}

	m.textInput, cmd = m.textInput.Update(msg)

	if m.query != m.textInput.Value() {
		m.query = m.textInput.Value()
		m.bookPanes, m.err = m.storygraph.Search(m.query)
	}

	return m, cmd
}

func (m BookAddModel) View() tea.View {
	var c *tea.Cursor
	if !m.textInput.VirtualCursor() {
		c = m.textInput.Cursor()
		c.Y += lipgloss.Height(m.headerView())
	}

	str := lipgloss.JoinVertical(lipgloss.Top, m.headerView(), m.textInput.View(), m.bookPanesView(), m.footerView())

	v := tea.NewView(str)
	v.Cursor = c
	return v
}

func (m BookAddModel) headerView() string { return "Search for the book you are adding" }
func (m BookAddModel) bookPanesView() string {
	var panes []string
	for _, pane := range m.bookPanes {
		panes = append(panes, pane.Title)
	}
	return lipgloss.JoinVertical(lipgloss.Top, panes...)
}
func (m BookAddModel) footerView() string {
	s := "\n(esc to quit)"
	if m.err != nil {
		s += " - " + m.err.Error()
	}
	return s
}

func (m *Manager) Add(paths []string) error {
	// Check if files exists
	for _, path := range paths {
		_, err := os.Stat(path)
		if err != nil {
			m.slog.Error("Failed to stat file", "path", path, "error", err)
			return err
		}
	}

	model, err := initialBookAddModel()
	if err != nil {
		m.slog.Error("Failed to initialize BookAddModel", "error", err)
	}

	p := tea.NewProgram(model)
	_, err = p.Run()
	if err != nil {
		m.slog.Error("Failed to start TUI program", "error", err)
	}

	return nil
}
