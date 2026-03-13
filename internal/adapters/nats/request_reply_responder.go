package nats

import (
	"context"
	"fmt"

	"github.com/nats-io/nats.go"
)

type RequestReplyResponder struct {
	url    string
	routes []ControlRoute
	nc     *nats.Conn
	subs   []*nats.Subscription
}

func NewRequestReplyResponder(url string, routes []ControlRoute) *RequestReplyResponder {
	return &RequestReplyResponder{
		url:    url,
		routes: append([]ControlRoute(nil), routes...),
	}
}

func (r *RequestReplyResponder) Start() error {
	if r == nil {
		return fmt.Errorf("request/reply responder is required")
	}
	if len(r.routes) == 0 {
		return fmt.Errorf("request/reply routes are required")
	}

	nc, err := connect(r.url)
	if err != nil {
		return err
	}

	subs := make([]*nats.Subscription, 0, len(r.routes))
	for _, route := range r.routes {
		route := route
		sub, err := nc.QueueSubscribe(route.Spec.Subject, route.Spec.QueueGroup, func(msg *nats.Msg) {
			if msg.Reply == "" {
				return
			}

			reply, replyErr := route.Handler(context.Background(), msg.Data)
			if replyErr != nil {
				return
			}

			_ = msg.Respond(reply)
		})
		if err != nil {
			for _, sub := range subs {
				_ = sub.Drain()
			}
			nc.Close()
			return fmt.Errorf("subscribe %s: %w", route.Spec.Subject, err)
		}
		subs = append(subs, sub)
	}

	r.nc = nc
	r.subs = subs
	return nc.Flush()
}

func (r *RequestReplyResponder) Close() error {
	if r == nil {
		return nil
	}

	for _, sub := range r.subs {
		if sub != nil {
			if err := sub.Drain(); err != nil {
				return err
			}
		}
	}

	if r.nc != nil {
		r.nc.Close()
	}

	return nil
}
