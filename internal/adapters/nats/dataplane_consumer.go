package nats

import (
	"context"
	"fmt"

	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
)

type DataPlaneMessageHandler interface {
	HandleDataPlaneMessage(context.Context, dataplaneapp.Message) *problem.Problem
}

type DataPlaneConsumer struct {
	url      string
	spec     ConsumerSpec
	handler  DataPlaneMessageHandler
	nc       *nats.Conn
	js       jetstream.JetStream
	consumer jetstream.Consumer
	cctx     jetstream.ConsumeContext
}

func NewDataPlaneConsumer(url string, spec ConsumerSpec, handler DataPlaneMessageHandler) *DataPlaneConsumer {
	return &DataPlaneConsumer{
		url:     url,
		spec:    spec,
		handler: handler,
	}
}

func (c *DataPlaneConsumer) Start() error {
	if c == nil || c.handler == nil {
		return fmt.Errorf("data plane message handler is required")
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
		return fmt.Errorf("ensure data plane stream: %w", err)
	}

	stream, err := js.Stream(ctx, c.spec.Event.Stream.Name)
	if err != nil {
		nc.Close()
		return fmt.Errorf("get data plane stream: %w", err)
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
		return fmt.Errorf("create data plane consumer: %w", err)
	}

	cctx, err := consumer.Consume(func(msg jetstream.Msg) {
		env, prob := decodeEvent[dataplaneapp.Message](c.spec.Event, msg.Data())
		if prob != nil {
			_ = msg.Term()
			return
		}

		handlerCtx := requestctx.WithCorrelationID(context.Background(), env.CorrelationID)
		if prob := c.handler.HandleDataPlaneMessage(handlerCtx, env.Payload); prob != nil {
			if prob.Retryable || prob.Code == problem.Unavailable || prob.Code == problem.Internal {
				_ = msg.Nak()
				return
			}
			_ = msg.Term()
			return
		}

		_ = msg.Ack()
	})
	if err != nil {
		nc.Close()
		return fmt.Errorf("consume data plane events: %w", err)
	}

	c.nc = nc
	c.js = js
	c.consumer = consumer
	c.cctx = cctx
	return nil
}

func (c *DataPlaneConsumer) Close() error {
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
