package consumer

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"strings"
	"time"

	actorcommon "internal/actors/common"
	adapterkafka "internal/adapters/kafka"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type KafkaTopicConsumerConfig struct {
	Kafka          settings.KafkaConfig
	Topic          string
	RouterPID      *actor.PID
	RequestTimeout time.Duration
}

type KafkaTopicConsumerActor struct {
	cfg      KafkaTopicConsumerConfig
	logger   *slog.Logger
	engine   *actor.Engine
	consumer *adapterkafka.TopicConsumer
	cancel   context.CancelFunc
}

func NewKafkaTopicConsumerActor(cfg KafkaTopicConsumerConfig) actor.Producer {
	return func() actor.Receiver {
		return &KafkaTopicConsumerActor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func (a *KafkaTopicConsumerActor) Receive(c *actor.Context) {
	if a.engine == nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		if err := a.start(c); err != nil {
			c.Send(c.Parent(), kafkaTopicConsumerFailedMessage{Topic: a.cfg.Topic, Err: err})
			c.Engine().Poison(c.PID())
		}
	case actor.Stopped:
		if a.cancel != nil {
			a.cancel()
		}
		if a.consumer != nil {
			if err := a.consumer.Close(); err != nil {
				a.logger.Error("close kafka topic consumer", "topic", a.cfg.Topic, "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("kafka topic consumer: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *KafkaTopicConsumerActor) start(c *actor.Context) error {
	if a.cfg.RouterPID == nil {
		return fmt.Errorf("topic router is required")
	}

	consumer, err := adapterkafka.NewTopicConsumer(adapterkafka.TopicConsumerConfig{
		Brokers:     a.cfg.Kafka.Brokers,
		GroupID:     defaultValue(a.cfg.Kafka.ConsumerGroup, "quality-service-consumer-v1"),
		ClientID:    defaultValue(a.cfg.Kafka.ClientID, "quality-service-consumer"),
		Topic:       a.cfg.Topic,
		MaxWait:     time.Second,
		DialTimeout: a.cfg.Kafka.DialTimeoutDuration(),
	}, adapterkafka.MessageHandlerFunc(func(ctx context.Context, message adapterkafka.Message) error {
		return a.handleMessage(ctx, message)
	}))
	if err != nil {
		return err
	}

	runCtx, cancel := context.WithCancel(context.Background())
	a.cancel = cancel
	a.consumer = consumer
	parent := c.Parent()
	engine := c.Engine()

	go func() {
		if err := consumer.Start(runCtx); err != nil && !errors.Is(err, context.Canceled) {
			engine.Send(parent, kafkaTopicConsumerFailedMessage{Topic: a.cfg.Topic, Err: err})
		}
	}()

	return nil
}

func (a *KafkaTopicConsumerActor) handleMessage(_ context.Context, message adapterkafka.Message) error {
	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	result, err := a.engine.Request(a.cfg.RouterPID, routeKafkaMessageMessage{
		Message:    message,
		IngestedAt: time.Now().UTC(),
	}, timeout).Result()
	if err != nil {
		return err
	}

	reply, ok := result.(routeKafkaMessageResult)
	if !ok {
		return fmt.Errorf("topic router response is invalid")
	}
	if reply.Prob != nil {
		return reply.Prob
	}
	return nil
}

func defaultValue(value, fallback string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return fallback
	}
	return value
}
