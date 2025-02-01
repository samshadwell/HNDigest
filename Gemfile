# frozen_string_literal: true

source 'https://rubygems.org'

git_source(:github) { |repo_name| "https://github.com/#{repo_name}" }

gem 'aws-sdk-dynamodb', '~> 1.135'
gem 'aws-sdk-ses', '~> 1.79'
gem 'http', '~> 5.2'
gem 'nokogiri', '~> 1.18' # Peer requirement of aws-sdk

group :development do
  gem 'pry-byebug', '~> 3.10'
  gem 'rubocop', '~> 1.71', require: false
end
