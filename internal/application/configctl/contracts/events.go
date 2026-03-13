package contracts

import "internal/shared/events"

const RuntimeUpdatedEventName events.Name = "configctl.runtime.updated"

type RuntimeUpdatedEvent struct {
	Metadata events.Metadata `json:"metadata"`
	Snapshot RuntimeSnapshot `json:"snapshot"`
}

func NewRuntimeUpdatedEvent(snapshot RuntimeSnapshot) RuntimeUpdatedEvent {
	return RuntimeUpdatedEvent{
		Metadata: events.NewMetadata(),
		Snapshot: snapshot,
	}
}

func (e RuntimeUpdatedEvent) EventName() events.Name {
	return RuntimeUpdatedEventName
}

func (e RuntimeUpdatedEvent) EventMetadata() events.Metadata {
	return e.Metadata
}
