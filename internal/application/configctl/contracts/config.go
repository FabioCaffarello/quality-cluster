package contracts

import (
	"time"

	"internal/shared/events"
)

type ConfigRecord struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Format    string    `json:"format"`
	Content   string    `json:"content"`
	Status    string    `json:"status"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

type RuntimeSnapshot struct {
	Version        int64          `json:"version"`
	ActiveConfigID string         `json:"active_config_id,omitempty"`
	Configs        []ConfigRecord `json:"configs"`
}

type RuntimeEvent interface {
	events.Event
}
