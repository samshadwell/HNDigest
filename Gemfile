# frozen_string_literal: true

source 'https://rubygems.org'

git_source(:github) { |repo_name| "https://github.com/#{repo_name}" }

gem 'aws-sdk-dynamodb', '~> 1.126'
gem 'aws-sdk-ses', '~> 1.76'
gem 'http', '~> 5.2'
gem 'nokogiri', '~> 1.16', '>= 1.16.7' # Peer requirement of aws-sdk

group :development do
  gem 'pry-byebug', '~> 3.10'
  gem 'rubocop', '~> 1.68', require: false
end
