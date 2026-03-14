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
	CreateDraft                 ControlSpec
	GetConfig                   ControlSpec
	GetActive                   ControlSpec
	ListActiveIngestionBindings ControlSpec
	ListConfigs                 ControlSpec
	ValidateDraft               ControlSpec
	ValidateConfig              ControlSpec
	CompileConfig               ControlSpec
	ActivateConfig              ControlSpec
	DraftCreated                EventSpec
	Validated                   EventSpec
	Compiled                    EventSpec
	Activated                   EventSpec
	Deactivated                 EventSpec
	IngestionRuntimeChanged     EventSpec
	Archived                    EventSpec
	Rejected                    EventSpec
	ValidatorRuntime            ConsumerSpec
}

func DefaultConfigctlRegistry() ConfigctlRegistry {
	eventStream := StreamSpec{
		Name:     "CONFIGCTL_EVENTS",
		Subjects: []string{"configctl.events.config.>"},
		Storage:  jetstream.FileStorage,
		MaxAge:   24 * time.Hour,
		MaxBytes: 256 * 1024 * 1024,
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
		ListActiveIngestionBindings: ControlSpec{
			Subject:     "configctl.control.list_active_ingestion_bindings",
			RequestType: "configctl.query.list_active_ingestion_bindings",
			ReplyType:   "configctl.reply.list_active_ingestion_bindings",
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
		ValidateConfig: ControlSpec{
			Subject:     "configctl.control.validate_config",
			RequestType: "configctl.command.validate_config",
			ReplyType:   "configctl.reply.validate_config",
			QueueGroup:  "configctl.control",
		},
		CompileConfig: ControlSpec{
			Subject:     "configctl.control.compile_config",
			RequestType: "configctl.command.compile_config",
			ReplyType:   "configctl.reply.compile_config",
			QueueGroup:  "configctl.control",
		},
		ActivateConfig: ControlSpec{
			Subject:     "configctl.control.activate_config",
			RequestType: "configctl.command.activate_config",
			ReplyType:   "configctl.reply.activate_config",
			QueueGroup:  "configctl.control",
		},
		DraftCreated: EventSpec{
			Subject: "configctl.events.config.draft_created",
			Type:    "configctl.event.config.draft_created",
			Stream:  eventStream,
		},
		Validated: EventSpec{
			Subject: "configctl.events.config.validated",
			Type:    "configctl.event.config.validated",
			Stream:  eventStream,
		},
		Compiled: EventSpec{
			Subject: "configctl.events.config.compiled",
			Type:    "configctl.event.config.compiled",
			Stream:  eventStream,
		},
		Activated: EventSpec{
			Subject: "configctl.events.config.activated",
			Type:    "configctl.event.config.activated",
			Stream:  eventStream,
		},
		Deactivated: EventSpec{
			Subject: "configctl.events.config.deactivated",
			Type:    "configctl.event.config.deactivated",
			Stream:  eventStream,
		},
		IngestionRuntimeChanged: EventSpec{
			Subject: "configctl.events.config.ingestion_runtime_changed",
			Type:    "configctl.event.config.ingestion_runtime_changed",
			Stream:  eventStream,
		},
		Archived: EventSpec{
			Subject: "configctl.events.config.archived",
			Type:    "configctl.event.config.archived",
			Stream:  eventStream,
		},
		Rejected: EventSpec{
			Subject: "configctl.events.config.rejected",
			Type:    "configctl.event.config.rejected",
			Stream:  eventStream,
		},
		ValidatorRuntime: ConsumerSpec{
			Durable: "validator-runtime-cache-v1",
			Event: EventSpec{
				Subject: "configctl.events.config.activated",
				Type:    "configctl.event.config.activated",
				Stream:  eventStream,
			},
			AckWait:    30 * time.Second,
			MaxDeliver: 10,
		},
	}
}
