package nats

import (
	"time"

	dataplaneapp "internal/application/dataplane"

	"github.com/nats-io/nats.go/jetstream"
)

type DataPlaneRegistry struct {
	Ingested          DataPlaneEventSpec
	ValidatorIngested ConsumerSpec
}

type DataPlaneEventSpec struct {
	SubjectPrefix    string
	SubjectPattern   string
	Type             string
	Stream           StreamSpec
	ValidatorDurable string
}

func (r DataPlaneRegistry) EventSpec(subject string) EventSpec {
	return EventSpec{
		Subject: subject,
		Type:    r.Ingested.Type,
		Stream:  r.Ingested.Stream,
	}
}

func DefaultDataPlaneRegistry() DataPlaneRegistry {
	registry := dataplaneapp.DefaultRegistry()

	return DataPlaneRegistry{
		Ingested: DataPlaneEventSpec{
			SubjectPrefix:  registry.JetStream.Ingested.SubjectPrefix,
			SubjectPattern: registry.JetStream.Ingested.SubjectPattern,
			Type:           registry.JetStream.Ingested.EventType,
			Stream: StreamSpec{
				Name:     registry.JetStream.Ingested.Stream,
				Subjects: []string{registry.JetStream.Ingested.SubjectPattern},
				Storage:  jetstream.FileStorage,
				MaxAge:   24 * time.Hour,
				MaxBytes: 256 * 1024 * 1024,
			},
			ValidatorDurable: registry.JetStream.Ingested.ValidatorDurable,
		},
		ValidatorIngested: ConsumerSpec{
			Durable: registry.JetStream.Ingested.ValidatorDurable,
			Event: EventSpec{
				Subject: registry.JetStream.Ingested.SubjectPattern,
				Type:    registry.JetStream.Ingested.EventType,
				Stream: StreamSpec{
					Name:     registry.JetStream.Ingested.Stream,
					Subjects: []string{registry.JetStream.Ingested.SubjectPattern},
					Storage:  jetstream.FileStorage,
					MaxAge:   24 * time.Hour,
					MaxBytes: 256 * 1024 * 1024,
				},
			},
			AckWait:    30 * time.Second,
			MaxDeliver: 10,
		},
	}
}
