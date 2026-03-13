package nats

import (
	"fmt"
	"os"
	"strings"

	"github.com/nats-io/nats.go"
)

func connect(url string) (*nats.Conn, error) {
	natsURL := strings.TrimSpace(url)
	if natsURL == "" {
		natsURL = strings.TrimSpace(os.Getenv("NATS_URL"))
	}
	if natsURL == "" {
		return nil, fmt.Errorf("NATS_URL is not set")
	}

	nc, err := nats.Connect(natsURL)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to nats: %w", err)
	}

	return nc, nil
}
