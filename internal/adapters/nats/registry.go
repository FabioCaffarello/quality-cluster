package nats

import (
	"github.com/fxamacker/cbor/v2"
	"github.com/nats-io/nats.go/jetstream"
)

type NatsRegistry struct {
	Stream StreamType
	Fn func(msg []byte, meta *jetstream.MsgMetadata) error
}

func CreateStreamHandler[T any](fn func(*T, *jetstream.MsgMetadata) error) func(msg []byte, meta *jetstream.MsgMetadata) error {
	return func(msg []byte, meta *jetstream.MsgMetadata) error {
		v := new(T)
		if err := cbor.Unmarshal(msg, v); err != nil {
			return err
		}
		return fn(v, meta)
	}
}