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
}

func initialBookAddModel() (BookAddModel, error) {
	ti := textinput.New()
	ti.Focus()

	s, err := storygraph.New()
	if err != nil {
		return BookAddModel{}, err
	}

	return BookAddModel{
		storygraph: s,
		textInput:  ti,
		query:      "",
	}, nil
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
		case "backspace":
			if len(m.query) > 0 {
				m.query = m.query[:len(m.query)-1]
			}
		default:
			m.query += msg.Key().Text
		}
	}

	m.textInput, cmd = m.textInput.Update(msg)
	return m, cmd
}

func (m BookAddModel) View() tea.View {
	var c *tea.Cursor
	if !m.textInput.VirtualCursor() {
		c = m.textInput.Cursor()
		c.Y += lipgloss.Height(m.headerView())
	}

	str := lipgloss.JoinVertical(lipgloss.Top, m.headerView(), m.textInput.View(), m.footerView())

	v := tea.NewView(str)
	v.Cursor = c
	return v
}

func (m BookAddModel) headerView() string { return "Search for the book you are adding" }
func (m BookAddModel) footerView() string { return "\n(esc to quit)" }

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
