package kafka

import (
	"context"
	"fmt"
	"strings"
	"time"

	kafkago "github.com/segmentio/kafka-go"
)

type Producer struct {
	writer *kafkago.Writer
}

func NewProducer(brokers []string, clientID string, dialTimeout time.Duration) (*Producer, error) {
	brokers = normalizeValues(brokers)
	if len(brokers) == 0 {
		return nil, fmt.Errorf("at least one broker is required")
	}
	if dialTimeout <= 0 {
		dialTimeout = 10 * time.Second
	}

	return &Producer{
		writer: &kafkago.Writer{
			Addr:         kafkago.TCP(brokers...),
			Balancer:     &kafkago.LeastBytes{},
			RequiredAcks: kafkago.RequireOne,
			Transport: &kafkago.Transport{
				ClientID:    strings.TrimSpace(clientID),
				DialTimeout: dialTimeout,
			},
		},
	}, nil
}

func (p *Producer) Publish(ctx context.Context, topic string, key []byte, value []byte, headers map[string]string, timestamp time.Time) error {
	if p == nil || p.writer == nil {
		return fmt.Errorf("producer is unavailable")
	}
	topic = strings.TrimSpace(topic)
	if topic == "" {
		return fmt.Errorf("topic is required")
	}
	if timestamp.IsZero() {
		timestamp = time.Now().UTC()
	}

	return p.writer.WriteMessages(ctx, kafkago.Message{
		Topic:   topic,
		Key:     append([]byte(nil), key...),
		Value:   append([]byte(nil), value...),
		Headers: kafkaHeaders(headers),
		Time:    timestamp.UTC(),
	})
}

func (p *Producer) Close() error {
	if p == nil || p.writer == nil {
		return nil
	}
	return p.writer.Close()
}
