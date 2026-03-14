package kafka

import (
	"context"
	"fmt"
	"net"
	"sort"
	"strconv"
	"strings"
	"time"

	kafkago "github.com/segmentio/kafka-go"
)

func EnsureTopics(ctx context.Context, brokers []string, topics []string, dialTimeout time.Duration) error {
	brokers = normalizeValues(brokers)
	topics = normalizeValues(topics)
	if len(brokers) == 0 {
		return fmt.Errorf("at least one broker is required")
	}
	if len(topics) == 0 {
		return nil
	}
	if dialTimeout <= 0 {
		dialTimeout = 10 * time.Second
	}

	dialer := &kafkago.Dialer{Timeout: dialTimeout}
	conn, err := dialer.DialContext(ctx, "tcp", brokers[0])
	if err != nil {
		return fmt.Errorf("dial broker: %w", err)
	}
	defer conn.Close()

	controller, err := conn.Controller()
	if err != nil {
		return fmt.Errorf("lookup controller: %w", err)
	}

	controllerConn, err := dialer.DialContext(ctx, "tcp", net.JoinHostPort(controller.Host, strconv.Itoa(controller.Port)))
	if err != nil {
		return fmt.Errorf("dial controller: %w", err)
	}
	defer controllerConn.Close()

	configs := make([]kafkago.TopicConfig, 0, len(topics))
	for _, topic := range topics {
		configs = append(configs, kafkago.TopicConfig{
			Topic:             topic,
			NumPartitions:     1,
			ReplicationFactor: 1,
		})
	}

	if err := controllerConn.CreateTopics(configs...); err != nil {
		return fmt.Errorf("create topics: %w", err)
	}
	return nil
}

func normalizeValues(values []string) []string {
	if len(values) == 0 {
		return nil
	}

	seen := make(map[string]struct{}, len(values))
	normalized := make([]string, 0, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value == "" {
			continue
		}
		if _, exists := seen[value]; exists {
			continue
		}
		seen[value] = struct{}{}
		normalized = append(normalized, value)
	}
	sort.Strings(normalized)
	return normalized
}
