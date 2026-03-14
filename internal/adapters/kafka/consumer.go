package kafka

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"time"

	kafkago "github.com/segmentio/kafka-go"
)

type MessageHandler interface {
	HandleMessage(context.Context, Message) error
}

type MessageHandlerFunc func(context.Context, Message) error

func (fn MessageHandlerFunc) HandleMessage(ctx context.Context, message Message) error {
	return fn(ctx, message)
}

type TopicConsumerConfig struct {
	Brokers     []string
	GroupID     string
	ClientID    string
	Topic       string
	MinBytes    int
	MaxBytes    int
	MaxWait     time.Duration
	DialTimeout time.Duration
}

type TopicConsumer struct {
	reader  *kafkago.Reader
	handler MessageHandler
}

func NewTopicConsumer(cfg TopicConsumerConfig, handler MessageHandler) (*TopicConsumer, error) {
	if handler == nil {
		return nil, fmt.Errorf("message handler is required")
	}

	cfg.Brokers = normalizeValues(cfg.Brokers)
	cfg.GroupID = strings.TrimSpace(cfg.GroupID)
	cfg.Topic = strings.TrimSpace(cfg.Topic)
	if len(cfg.Brokers) == 0 {
		return nil, fmt.Errorf("at least one broker is required")
	}
	if cfg.GroupID == "" {
		return nil, fmt.Errorf("group id is required")
	}
	if cfg.Topic == "" {
		return nil, fmt.Errorf("topic is required")
	}
	if cfg.MinBytes <= 0 {
		cfg.MinBytes = 1
	}
	if cfg.MaxBytes <= 0 {
		cfg.MaxBytes = 10e6
	}
	if cfg.MaxWait <= 0 {
		cfg.MaxWait = time.Second
	}
	if cfg.DialTimeout <= 0 {
		cfg.DialTimeout = 10 * time.Second
	}

	reader := kafkago.NewReader(kafkago.ReaderConfig{
		Brokers:  cfg.Brokers,
		GroupID:  cfg.GroupID,
		Topic:    cfg.Topic,
		MinBytes: cfg.MinBytes,
		MaxBytes: cfg.MaxBytes,
		MaxWait:  cfg.MaxWait,
		Dialer: &kafkago.Dialer{
			ClientID: strings.TrimSpace(cfg.ClientID),
			Timeout:  cfg.DialTimeout,
		},
	})

	return &TopicConsumer{
		reader:  reader,
		handler: handler,
	}, nil
}

func (c *TopicConsumer) Start(ctx context.Context) error {
	if c == nil || c.reader == nil || c.handler == nil {
		return fmt.Errorf("topic consumer is unavailable")
	}

	for {
		record, err := c.reader.FetchMessage(ctx)
		if err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return nil
			}
			return fmt.Errorf("fetch kafka message: %w", err)
		}

		if err := c.handler.HandleMessage(ctx, messageFromRecord(record)); err != nil {
			return err
		}

		if err := c.reader.CommitMessages(ctx, record); err != nil {
			if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
				return nil
			}
			return fmt.Errorf("commit kafka message: %w", err)
		}
	}
}

func (c *TopicConsumer) Close() error {
	if c == nil || c.reader == nil {
		return nil
	}
	return c.reader.Close()
}
