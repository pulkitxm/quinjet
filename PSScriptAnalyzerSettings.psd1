@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        'PSAvoidGlobalVars',
        'PSReviewUnusedParameter',
        'PSUseSingularNouns',
        'PSAvoidUsingWriteHost',
        'PSUseShouldProcessForStateChangingFunctions'
    )
}
