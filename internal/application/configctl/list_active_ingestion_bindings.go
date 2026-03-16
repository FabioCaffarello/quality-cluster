package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type ListActiveIngestionBindingsUseCase struct {
	repository Repository
}

func NewListActiveIngestionBindingsUseCase(repository Repository) *ListActiveIngestionBindingsUseCase {
	return &ListActiveIngestionBindingsUseCase{repository: repository}
}

func (uc *ListActiveIngestionBindingsUseCase) Execute(ctx context.Context, query contracts.ListActiveIngestionBindingsQuery) (contracts.ListActiveIngestionBindingsReply, *problem.Problem) {
	if uc == nil || uc.repository == nil {
		return contracts.ListActiveIngestionBindingsReply{}, problem.New(problem.Unavailable, "config repository is unavailable")
	}

	query = query.Normalize()
	if prob := query.Validate(); prob != nil {
		return contracts.ListActiveIngestionBindingsReply{}, prob
	}

	runtimes, prob := uc.repository.ListIngestionRuntimes(ctx)
	if prob != nil {
		return contracts.ListActiveIngestionBindingsReply{}, prob
	}

	if query.ScopeKind != "" {
		scope := configdomain.ActivationScope{Kind: query.ScopeKind, Key: query.ScopeKey}.Normalize()
		filtered := make([]configdomain.IngestionRuntimeProjection, 0, len(runtimes))
		for _, runtime := range runtimes {
			if runtime.Scope.Normalize().String() == scope.String() {
				filtered = append(filtered, runtime)
			}
		}
		runtimes = filtered
	}

	return contracts.ListActiveIngestionBindingsReply{
		Bindings: activeIngestionBindingsFromDomain(runtimes),
		Runtimes: compactIngestionRuntimesFromDomain(runtimes),
	}, nil
}
