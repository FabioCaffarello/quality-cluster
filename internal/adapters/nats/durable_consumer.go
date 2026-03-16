package nats

import (
	"context"
	"fmt"
	"time"

	configdomain "internal/domain/configctl"
	"internal/shared/problem"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
)

const defaultSetupTimeout = 10 * time.Second

type ConfigActivatedHandler interface {
	HandleConfigActivated(context.Context, configdomain.ConfigActivatedEvent) *problem.Problem
}

type ConfigDeactivatedHandler interface {
	HandleConfigDeactivated(context.Context, configdomain.ConfigDeactivatedEvent) *problem.Problem
}

type IngestionRuntimeChangedHandler interface {
	HandleIngestionRuntimeChanged(context.Context, configdomain.IngestionRuntimeChangedEvent) *problem.Problem
}

type ConfigActivatedConsumer struct {
	url      string
	spec     ConsumerSpec
	handler  ConfigActivatedHandler
	nc       *nats.Conn
	js       jetstream.JetStream
	consumer jetstream.Consumer
	cctx     jetstream.ConsumeContext
}

func NewConfigActivatedConsumer(url string, spec ConsumerSpec, handler ConfigActivatedHandler) *ConfigActivatedConsumer {
	return &ConfigActivatedConsumer{
		url:     url,
		spec:    spec,
		handler: handler,
	}
}

func (c *ConfigActivatedConsumer) Start() error {
	if c == nil || c.handler == nil {
		return fmt.Errorf("config activated handler is required")
	}

	nc, err := connect(c.url)
	if err != nil {
		return err
	}

	js, err := jetstream.New(nc)
	if err != nil {
		nc.Close()
		return fmt.Errorf("create jetstream context: %w", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), defaultSetupTimeout)
	defer cancel()

	if _, err := js.CreateOrUpdateStream(ctx, c.spec.Event.Stream.Config()); err != nil {
		nc.Close()
		return fmt.Errorf("ensure config event stream: %w", err)
	}

	stream, err := js.Stream(ctx, c.spec.Event.Stream.Name)
	if err != nil {
		nc.Close()
		return fmt.Errorf("get config event stream: %w", err)
	}

	consumer, err := stream.CreateOrUpdateConsumer(ctx, jetstream.ConsumerConfig{
		Durable:        c.spec.Durable,
		FilterSubjects: []string{c.spec.Event.Subject},
		AckPolicy:      jetstream.AckExplicitPolicy,
		AckWait:        c.spec.AckWait,
		MaxDeliver:     c.spec.MaxDeliver,
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("create config event consumer: %w", err)
	}

	cctx, err := consumer.Consume(func(msg jetstream.Msg) {
		env, prob := decodeEvent[configdomain.ConfigActivatedEvent](c.spec.Event, msg.Data())
		if prob != nil {
			_ = msg.Term()
			return
		}

		if prob := c.handler.HandleConfigActivated(context.Background(), env.Payload); prob != nil {
			_ = msg.Nak()
			return
		}

		_ = msg.Ack()
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("consume config activated events: %w", err)
	}

	c.nc = nc
	c.js = js
	c.consumer = consumer
	c.cctx = cctx
	return nil
}

func (c *ConfigActivatedConsumer) Close() error {
	if c == nil {
		return nil
	}
	if c.cctx != nil {
		c.cctx.Stop()
	}
	if c.nc != nil {
		c.nc.Close()
	}
	return nil
}

type ConfigDeactivatedConsumer struct {
	url      string
	spec     ConsumerSpec
	handler  ConfigDeactivatedHandler
	nc       *nats.Conn
	js       jetstream.JetStream
	consumer jetstream.Consumer
	cctx     jetstream.ConsumeContext
}

func NewConfigDeactivatedConsumer(url string, spec ConsumerSpec, handler ConfigDeactivatedHandler) *ConfigDeactivatedConsumer {
	return &ConfigDeactivatedConsumer{
		url:     url,
		spec:    spec,
		handler: handler,
	}
}

func (c *ConfigDeactivatedConsumer) Start() error {
	if c == nil || c.handler == nil {
		return fmt.Errorf("config deactivated handler is required")
	}

	nc, err := connect(c.url)
	if err != nil {
		return err
	}

	js, err := jetstream.New(nc)
	if err != nil {
		nc.Close()
		return fmt.Errorf("create jetstream context: %w", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), defaultSetupTimeout)
	defer cancel()

	if _, err := js.CreateOrUpdateStream(ctx, c.spec.Event.Stream.Config()); err != nil {
		nc.Close()
		return fmt.Errorf("ensure config event stream: %w", err)
	}

	stream, err := js.Stream(ctx, c.spec.Event.Stream.Name)
	if err != nil {
		nc.Close()
		return fmt.Errorf("get config event stream: %w", err)
	}

	consumer, err := stream.CreateOrUpdateConsumer(ctx, jetstream.ConsumerConfig{
		Durable:        c.spec.Durable,
		FilterSubjects: []string{c.spec.Event.Subject},
		AckPolicy:      jetstream.AckExplicitPolicy,
		AckWait:        c.spec.AckWait,
		MaxDeliver:     c.spec.MaxDeliver,
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("create config event consumer: %w", err)
	}

	cctx, err := consumer.Consume(func(msg jetstream.Msg) {
		env, prob := decodeEvent[configdomain.ConfigDeactivatedEvent](c.spec.Event, msg.Data())
		if prob != nil {
			_ = msg.Term()
			return
		}

		if prob := c.handler.HandleConfigDeactivated(context.Background(), env.Payload); prob != nil {
			_ = msg.Nak()
			return
		}

		_ = msg.Ack()
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("consume config deactivated events: %w", err)
	}

	c.nc = nc
	c.js = js
	c.consumer = consumer
	c.cctx = cctx
	return nil
}

func (c *ConfigDeactivatedConsumer) Close() error {
	if c == nil {
		return nil
	}
	if c.cctx != nil {
		c.cctx.Stop()
	}
	if c.nc != nil {
		c.nc.Close()
	}
	return nil
}

type IngestionRuntimeChangedConsumer struct {
	url      string
	spec     ConsumerSpec
	handler  IngestionRuntimeChangedHandler
	nc       *nats.Conn
	js       jetstream.JetStream
	consumer jetstream.Consumer
	cctx     jetstream.ConsumeContext
}

func NewIngestionRuntimeChangedConsumer(url string, spec ConsumerSpec, handler IngestionRuntimeChangedHandler) *IngestionRuntimeChangedConsumer {
	return &IngestionRuntimeChangedConsumer{
		url:     url,
		spec:    spec,
		handler: handler,
	}
}

func (c *IngestionRuntimeChangedConsumer) Start() error {
	if c == nil || c.handler == nil {
		return fmt.Errorf("ingestion runtime changed handler is required")
	}

	nc, err := connect(c.url)
	if err != nil {
		return err
	}

	js, err := jetstream.New(nc)
	if err != nil {
		nc.Close()
		return fmt.Errorf("create jetstream context: %w", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), defaultSetupTimeout)
	defer cancel()

	if _, err := js.CreateOrUpdateStream(ctx, c.spec.Event.Stream.Config()); err != nil {
		nc.Close()
		return fmt.Errorf("ensure config event stream: %w", err)
	}

	stream, err := js.Stream(ctx, c.spec.Event.Stream.Name)
	if err != nil {
		nc.Close()
		return fmt.Errorf("get config event stream: %w", err)
	}

	consumer, err := stream.CreateOrUpdateConsumer(ctx, jetstream.ConsumerConfig{
		Durable:        c.spec.Durable,
		FilterSubjects: []string{c.spec.Event.Subject},
		AckPolicy:      jetstream.AckExplicitPolicy,
		AckWait:        c.spec.AckWait,
		MaxDeliver:     c.spec.MaxDeliver,
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("create ingestion runtime changed consumer: %w", err)
	}

	cctx, err := consumer.Consume(func(msg jetstream.Msg) {
		env, prob := decodeEvent[configdomain.IngestionRuntimeChangedEvent](c.spec.Event, msg.Data())
		if prob != nil {
			_ = msg.Term()
			return
		}

		if prob := c.handler.HandleIngestionRuntimeChanged(context.Background(), env.Payload); prob != nil {
			_ = msg.Nak()
			return
		}

		_ = msg.Ack()
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("consume ingestion runtime changed events: %w", err)
	}

	c.nc = nc
	c.js = js
	c.consumer = consumer
	c.cctx = cctx
	return nil
}

func (c *IngestionRuntimeChangedConsumer) Close() error {
	if c == nil {
		return nil
	}
	if c.cctx != nil {
		c.cctx.Stop()
	}
	if c.nc != nil {
		c.nc.Close()
	}
	return nil
}
