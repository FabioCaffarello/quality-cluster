package nats

import (
	"context"
	"fmt"
	"time"

	"internal/application/configctl/contracts"
	"internal/shared/problem"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
)

const defaultSetupTimeout = 10 * time.Second

type RuntimeUpdatedHandler interface {
	HandleRuntimeUpdated(context.Context, contracts.RuntimeUpdatedEvent) *problem.Problem
}

type RuntimeUpdatedConsumer struct {
	url      string
	spec     ConsumerSpec
	handler  RuntimeUpdatedHandler
	nc       *nats.Conn
	js       jetstream.JetStream
	consumer jetstream.Consumer
	cctx     jetstream.ConsumeContext
}

func NewRuntimeUpdatedConsumer(url string, spec ConsumerSpec, handler RuntimeUpdatedHandler) *RuntimeUpdatedConsumer {
	return &RuntimeUpdatedConsumer{
		url:     url,
		spec:    spec,
		handler: handler,
	}
}

func (c *RuntimeUpdatedConsumer) Start() error {
	if c == nil || c.handler == nil {
		return fmt.Errorf("runtime updated handler is required")
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
		return fmt.Errorf("ensure runtime stream: %w", err)
	}

	stream, err := js.Stream(ctx, c.spec.Event.Stream.Name)
	if err != nil {
		nc.Close()
		return fmt.Errorf("get runtime stream: %w", err)
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
		return fmt.Errorf("create runtime consumer: %w", err)
	}

	cctx, err := consumer.Consume(func(msg jetstream.Msg) {
		env, prob := decodeRuntimeEvent[contracts.RuntimeUpdatedEvent](c.spec.Event, msg.Data())
		if prob != nil {
			_ = msg.Term()
			return
		}

		if prob := c.handler.HandleRuntimeUpdated(context.Background(), env.Payload); prob != nil {
			_ = msg.Nak()
			return
		}

		_ = msg.Ack()
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("consume runtime updates: %w", err)
	}

	c.nc = nc
	c.js = js
	c.consumer = consumer
	c.cctx = cctx
	return nil
}

func (c *RuntimeUpdatedConsumer) Close() error {
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
