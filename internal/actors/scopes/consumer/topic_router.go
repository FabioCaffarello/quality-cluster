package consumer

import (
	"fmt"
	"log/slog"
	"time"

	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type TopicRouterConfig struct {
	Topic          dataplaneapp.TopicTopology
	PublisherPID   *actor.PID
	RequestTimeout time.Duration
}

type TopicRouterActor struct {
	cfg          TopicRouterConfig
	logger       *slog.Logger
	publisherPID *actor.PID
}

func NewTopicRouterActor(cfg TopicRouterConfig) actor.Producer {
	return func() actor.Receiver {
		return &TopicRouterActor{
			cfg:          cfg,
			logger:       slog.Default(),
			publisherPID: cfg.PublisherPID,
		}
	}
}

func (a *TopicRouterActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case routeKafkaMessageMessage:
		c.Respond(routeKafkaMessageResult{Prob: a.routeMessage(c, msg)})
	case actor.Stopped:
	default:
		a.logger.Warn("consumer topic router: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *TopicRouterActor) routeMessage(c *actor.Context, msg routeKafkaMessageMessage) *problem.Problem {
	if a.publisherPID == nil {
		return problem.New(problem.Unavailable, "data plane publisher actor is unavailable").MarkRetryable()
	}

	record, prob := dataplaneapp.NewKafkaRecord(
		msg.Message.Topic,
		msg.Message.Key,
		msg.Message.Value,
		msg.Message.Headers,
		msg.Message.Partition,
		msg.Message.Offset,
		msg.Message.Timestamp,
	)
	if prob != nil {
		return prob
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	for _, binding := range a.cfg.Topic.Bindings {
		routed, mapProb := dataplaneapp.MapKafkaRecordToBinding(binding, record, msg.IngestedAt)
		if mapProb != nil {
			a.logger.Warn("dropping kafka message during mapping", "topic", msg.Message.Topic, "offset", msg.Message.Offset, "error", mapProb)
			continue
		}

		result, err := c.Request(a.publisherPID, publishRoutedMessageMessage{Message: routed}, timeout).Result()
		if err != nil {
			return problem.Wrap(err, problem.Unavailable, "publish data plane message").MarkRetryable()
		}

		reply, ok := result.(publishRoutedMessageResult)
		if !ok {
			return problem.New(problem.Internal, "publisher actor response is invalid")
		}
		if reply.Prob != nil {
			return reply.Prob
		}
	}

	return nil
}
