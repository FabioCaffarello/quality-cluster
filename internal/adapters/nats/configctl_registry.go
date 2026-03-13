package nats

import (
	"time"

	"github.com/nats-io/nats.go/jetstream"
)

type ControlSpec struct {
	Subject     string
	RequestType string
	ReplyType   string
	QueueGroup  string
}

type StreamSpec struct {
	Name     string
	Subjects []string
	Storage  jetstream.StorageType
	MaxAge   time.Duration
	MaxBytes int64
}

func (s StreamSpec) Config() jetstream.StreamConfig {
	return jetstream.StreamConfig{
		Name:       s.Name,
		Subjects:   append([]string(nil), s.Subjects...),
		Storage:    s.Storage,
		MaxAge:     s.MaxAge,
		MaxBytes:   s.MaxBytes,
		MaxMsgSize: 10 * 1024 * 1024,
	}
}

type EventSpec struct {
	Subject string
	Type    string
	Stream  StreamSpec
}

type ConsumerSpec struct {
	Durable    string
	Event      EventSpec
	AckWait    time.Duration
	MaxDeliver int
}

type ConfigctlRegistry struct {
	CreateDraft    ControlSpec
	GetConfig      ControlSpec
	GetActive      ControlSpec
	ListConfigs    ControlSpec
	ValidateDraft  ControlSpec
	RuntimeUpdated EventSpec
	ValidatorCache ConsumerSpec
}

func DefaultConfigctlRegistry() ConfigctlRegistry {
	runtimeStream := StreamSpec{
		Name:     "CONFIGCTL_RUNTIME",
		Subjects: []string{"configctl.events.runtime.>"},
		Storage:  jetstream.FileStorage,
		MaxAge:   24 * time.Hour,
		MaxBytes: 256 * 1024 * 1024,
	}

	runtimeUpdated := EventSpec{
		Subject: "configctl.events.runtime.updated",
		Type:    "configctl.event.runtime.updated",
		Stream:  runtimeStream,
	}

	return ConfigctlRegistry{
		CreateDraft: ControlSpec{
			Subject:     "configctl.control.create_draft",
			RequestType: "configctl.command.create_draft",
			ReplyType:   "configctl.reply.create_draft",
			QueueGroup:  "configctl.control",
		},
		GetConfig: ControlSpec{
			Subject:     "configctl.control.get_config",
			RequestType: "configctl.query.get_config",
			ReplyType:   "configctl.reply.get_config",
			QueueGroup:  "configctl.control",
		},
		GetActive: ControlSpec{
			Subject:     "configctl.control.get_active",
			RequestType: "configctl.query.get_active",
			ReplyType:   "configctl.reply.get_active",
			QueueGroup:  "configctl.control",
		},
		ListConfigs: ControlSpec{
			Subject:     "configctl.control.list_configs",
			RequestType: "configctl.query.list_configs",
			ReplyType:   "configctl.reply.list_configs",
			QueueGroup:  "configctl.control",
		},
		ValidateDraft: ControlSpec{
			Subject:     "configctl.control.validate_draft",
			RequestType: "configctl.command.validate_draft",
			ReplyType:   "configctl.reply.validate_draft",
			QueueGroup:  "configctl.control",
		},
		RuntimeUpdated: runtimeUpdated,
		ValidatorCache: ConsumerSpec{
			Durable:    "validator-runtime-cache-v1",
			Event:      runtimeUpdated,
			AckWait:    30 * time.Second,
			MaxDeliver: 10,
		},
	}
}
