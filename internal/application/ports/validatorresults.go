package ports

import (
	"context"

	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
)

type ValidatorResultsGateway interface {
	ListValidationResults(context.Context, validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem)
}
