package configctl

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"time"

	"internal/application/configctl/contracts"
	"internal/domain/configuration"
	"internal/shared/problem"
	"internal/shared/requestctx"
)

type CreateDraftUseCase struct {
	repository Repository
	publisher  RuntimeEventPublisher
	now        func() time.Time
	nextID     func() string
}

func NewCreateDraftUseCase(repository Repository, publisher RuntimeEventPublisher) *CreateDraftUseCase {
	return &CreateDraftUseCase{
		repository: repository,
		publisher:  publisher,
		now: func() time.Time {
			return time.Now().UTC()
		},
		nextID: newID,
	}
}

func (uc *CreateDraftUseCase) Execute(ctx context.Context, command contracts.CreateDraftCommand) (contracts.CreateDraftReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.CreateDraftReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	command = command.Normalize()
	if prob := command.Validate(); prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	now := uc.now()
	config, prob := configuration.NewDraft(
		uc.nextID(),
		command.Name,
		configuration.Format(command.Format),
		command.Content,
		now,
	)
	if prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	if prob := uc.repository.SaveDraft(ctx, config); prob != nil {
		return contracts.CreateDraftReply{}, prob
	}

	snapshot, prob := uc.repository.Snapshot(ctx)
	if prob != nil {
		_ = uc.repository.Delete(ctx, config.ID)
		return contracts.CreateDraftReply{}, prob
	}

	if uc.publisher != nil {
		runtimeEvent := contracts.NewRuntimeUpdatedEvent(snapshot)
		runtimeEvent.Metadata = runtimeEvent.Metadata.WithCorrelationID(requestctx.CorrelationID(ctx))
		if prob := uc.publisher.Publish(ctx, runtimeEvent); prob != nil {
			_ = uc.repository.Delete(ctx, config.ID)
			return contracts.CreateDraftReply{}, prob
		}
	}

	return contracts.CreateDraftReply{
		Config: recordFromDomain(config),
	}, nil
}

func newID() string {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return time.Now().UTC().Format("20060102150405.000000000")
	}
	return hex.EncodeToString(raw[:])
}
